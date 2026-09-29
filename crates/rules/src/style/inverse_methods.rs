//! `Style/InverseMethods`, ported from RuboCop's
//! `lib/rubocop/cop/style/inverse_methods.rb`.
//!
//! # Two independent shapes, one entry point
//!
//! Upstream dispatches through `on_send`/`on_csend` (a `!`-named call whose
//! receiver is itself a method call, possibly parenthesized) and
//! `on_block`/`on_numblock`/`on_itblock` (a `select`/`reject`-like call whose
//! block's last statement negates its value) separately. Prism folds a call
//! and its attached block (literal or `&:sym`/`&block` pass) into one
//! [`NodeKind::CallNode`] -- [`InverseMethods::enter`] therefore subscribes
//! to that one kind and dispatches on the call's own name: `!`
//! ([`InverseMethods::check_inverse_method`]) or anything else that might be
//! a `select`/`reject`-like call ([`InverseMethods::check_inverse_block`]).
//! `CallNode::block` also yields a `BlockArgumentNode` for `&:sym`/`&block`;
//! only a literal [`NodeKind::BlockNode`] (`as_block_node`) is a candidate
//! for [`InverseMethods::check_inverse_block`].
//!
//! # `(begin (call ...))` vs Prism's always-present `StatementsNode`
//!
//! Whitequark elides the wrapping `begin` node for a single-statement
//! parenthesized group or block body; Prism never does; both bodies always
//! sit inside a [`NodeKind::StatementsNode`]. `!(foo.bar)`'s parens are a
//! [`NodeKind::ParenthesesNode`] whose single-statement body is unwrapped by
//! [`unwrap_single_statement_parens`]; a block's body's *last* statement
//! (whether the body has one statement or several) is read directly off
//! [`ruby_ast::node::StatementsNode::body`] in [`InverseMethods::check_inverse_block`],
//! collapsing upstream's single-statement/`begin`-node alternation into one
//! path.
//!
//! # `negated?` without parent node access
//!
//! Upstream's `negated?(node)` reads `node.parent.method?(:!)` -- a live
//! parent node. [`Context::ancestors`] gives only kind and span, so
//! [`ancestor_is_bang`] instead recognizes a `!`/`not` ancestor from its
//! *text*: a prefix `!`, `not`, or `!!`/`not not`-chained call is the only
//! shape whose own source span can start with the byte `'!'` or the keyword
//! `not` (an infix `!=`/`!~` call's span starts at its receiver, never at
//! the operator), so this is exact, not a heuristic approximation.
//!
//! # `possible_class_hierarchy_check?`
//!
//! `!(Integer < Numeric)` is not the same as `Integer > Numeric` when
//! comparing classes, so a `<`/`<=`/`>`/`>=` comparison with a camel-case
//! constant operand is left alone. [`is_camel_case_constant`] ports
//! upstream's `CAMEL_CASE` regex (`/[A-Z]+[a-z]+/`) as a direct byte scan
//! rather than pulling in a regex dependency for one fixed pattern.

use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, NodeInfo, OptionError,
    OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `CLASS_COMPARISON_METHODS`.
const CLASS_COMPARISON_METHODS: &[&[u8]] = &[b"<=", b">=", b"<", b">"];

/// RuboCop's `SAFE_NAVIGATION_INCOMPATIBLE_METHODS`
/// (`CLASS_COMPARISON_METHODS + %i[any? none?]`).
const SAFE_NAVIGATION_INCOMPATIBLE_METHODS: &[&[u8]] =
    &[b"<=", b">=", b"<", b">", b"any?", b"none?"];

/// RuboCop's `EQUALITY_METHODS`.
const EQUALITY_METHODS: &[&[u8]] = &[b"==", b"!=", b"=~", b"!~", b"<=", b">=", b"<", b">"];

/// `config/default.yml`'s `Style/InverseMethods/InverseMethods`.
const DEFAULT_INVERSE_METHODS: &[(&str, &str)] =
    &[("any?", "none?"), ("even?", "odd?"), ("==", "!="), ("=~", "!~"), ("<", ">="), (">", "<=")];

/// `config/default.yml`'s `Style/InverseMethods/InverseBlocks`.
const DEFAULT_INVERSE_BLOCKS: &[(&str, &str)] = &[("select", "reject"), ("select!", "reject!")];

/// Strips a single leading `:` -- `config/default.yml` writes
/// `InverseMethods`/`InverseBlocks` keys and values as literal Ruby symbols
/// (`:any?: :none?`), and this crate's YAML loader folds a symbol scalar
/// into its bare string *including* that leading colon (see
/// [`config::YamlValue`]'s doc comment), unlike a fixture's own `.yml`
/// override, which spells the same option as a plain string (`any?:
/// none?`). Stripping a colon that was never there is a no-op, so this
/// handles both spellings uniformly.
fn strip_symbol_colon(s: &str) -> &str {
    s.strip_prefix(':').unwrap_or(s)
}

/// Reads `key`'s configured mapping (falling back to `defaults`) and returns
/// it merged with its own inversion, matching upstream's
/// `cop_config['InverseMethods'].merge(cop_config['InverseMethods'].invert)`.
fn bidirectional_map(
    options: &RuleOptions,
    key: &str,
    defaults: &[(&str, &str)],
) -> HashMap<Vec<u8>, String> {
    let pairs: Vec<(String, String)> = match options.get(key).and_then(OptionValue::as_map) {
        Some(map) => map
            .iter()
            .filter_map(|(k, v)| {
                v.as_str().map(|vs| {
                    (strip_symbol_colon(k).to_string(), strip_symbol_colon(vs).to_string())
                })
            })
            .collect(),
        None => defaults.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect(),
    };
    let mut result = HashMap::with_capacity(pairs.len() * 2);
    for (k, v) in &pairs {
        result.insert(k.as_bytes().to_vec(), v.clone());
        result.insert(v.as_bytes().to_vec(), k.clone());
    }
    result
}

/// RuboCop's `CAMEL_CASE` regex (`/[A-Z]+[a-z]+/`): an uppercase run
/// immediately followed by a lowercase letter, anywhere in `text`.
fn has_camel_case_run(text: &[u8]) -> bool {
    let mut i = 0;
    while i < text.len() {
        if text[i].is_ascii_uppercase() {
            let mut j = i + 1;
            while j < text.len() && text[j].is_ascii_uppercase() {
                j += 1;
            }
            if text.get(j).is_some_and(u8::is_ascii_lowercase) {
                return true;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    false
}

/// Upstream's `camel_case_constant?`: a constant reference (bare or
/// qualified) whose source matches [`has_camel_case_run`].
fn is_camel_case_constant(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let is_const = node.as_constant_read_node().is_some() || node.as_constant_path_node().is_some();
    is_const && has_camel_case_run(ctx.text(node.span()))
}

/// Upstream's `possible_class_hierarchy_check?`.
fn possible_class_hierarchy_check(
    method: &[u8],
    lhs: &Node<'_>,
    method_call: &CallNode<'_>,
    ctx: &Context<'_>,
) -> bool {
    if !CLASS_COMPARISON_METHODS.contains(&method) {
        return false;
    }
    if is_camel_case_constant(lhs, ctx) {
        return true;
    }
    let Some(args) = method_call.arguments() else { return false };
    let list = args.arguments();
    if list.len() != 1 {
        return false;
    }
    let rhs = list.first().expect("checked len == 1 above");
    is_camel_case_constant(&rhs, ctx)
}

/// `(begin (call ...))`'s Prism shape: a `ParenthesesNode` wrapping exactly
/// one statement, unwrapped to that statement.
fn unwrap_single_statement_parens<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let parens = node.as_parentheses_node()?;
    let stmts = parens.body()?.as_statements_node()?;
    let list = stmts.body();
    if list.len() != 1 {
        return None;
    }
    list.first()
}

/// A prefix `!`/`not`, or a chain of them (`!!`, `not not`): the only shapes
/// whose own source span can start with the byte `'!'` or the keyword
/// `not` (see the module doc's "`negated?` without parent node access").
fn is_bang_text(text: &[u8]) -> bool {
    if text.first() == Some(&b'!') {
        return true;
    }
    text.starts_with(b"not")
        && text.get(3).is_none_or(|b| !(b.is_ascii_alphanumeric() || *b == b'_'))
}

/// Whether the ancestor `levels` steps up from the node currently being
/// entered (1 = immediate parent) is a `!`/`not` call.
fn ancestor_is_bang(ctx: &Context<'_>, levels: usize) -> bool {
    let ancestors = ctx.ancestors();
    let Some(idx) = ancestors.len().checked_sub(levels) else { return false };
    let info: NodeInfo = ancestors[idx];
    info.kind == NodeKind::CallNode && is_bang_text(ctx.text(info.span))
}

/// Upstream's `node.each_node(:next).any?`: whether `node`'s subtree
/// (including `node` itself) contains a `next`.
fn contains_next(node: &Node<'_>) -> bool {
    if node.as_next_node().is_some() {
        return true;
    }
    let mut found = false;
    for_each_child(node, |child| {
        found = found || contains_next(child);
    });
    found
}

/// Use the inverse method instead of `!.method` if an inverse method is defined.
#[derive(Debug, Clone)]
pub struct InverseMethods {
    /// `InverseMethods`, merged with its own inversion.
    inverse_method_map: HashMap<Vec<u8>, String>,
    /// `InverseBlocks`, merged with its own inversion.
    inverse_blocks: HashMap<Vec<u8>, String>,
    /// Span starts of `!` leaves already folded into a reported
    /// [`InverseMethods::check_inverse_block`] offense -- upstream's
    /// `ignore_node(block)`/`part_of_ignored_node?`, ported as an explicit
    /// skip set since this engine has no per-file `IgnoredNode` mechanism:
    /// without it, `y.select { |key, _value| !(key =~ /c\d/) }`'s inner
    /// `!(key =~ /c\d/)` would *also* independently match
    /// [`InverseMethods::check_inverse_method`] (visited afterwards, being
    /// a descendant) and register a second, spurious `!~` offense.
    suppressed: std::collections::HashSet<u32>,
}

impl InverseMethods {
    /// Upstream's `on_send`/`on_csend`: a `!`/`not` call whose receiver is a
    /// method call (directly, or through a single-statement parenthesized
    /// group) with an explicit receiver of its own.
    fn check_inverse_method(
        &mut self,
        call: &CallNode<'_>,
        node: &Node<'_>,
        ctx: &mut Context<'_>,
    ) {
        if self.suppressed.remove(&node.span().start) {
            return;
        }
        if call.call_operator_loc().is_some() {
            // A postfix `BasicObject#!` (`e.bar?.!`), not a negation.
            return;
        }
        let Some(receiver) = call.receiver() else { return };

        let (method_call_node, wrapped_in_parens) = match unwrap_single_statement_parens(&receiver)
        {
            Some(inner) => (inner, true),
            None => (receiver, false),
        };
        let Some(method_call) = method_call_node.as_call_node() else { return };
        if method_call.receiver().is_none() {
            return;
        }
        let method = method_call.name().as_slice();
        let Some(inverse) = self.inverse_method_map.get(method) else { return };

        if ancestor_is_bang(ctx, 1) {
            return;
        }
        if method_call.is_safe_navigation()
            && SAFE_NAVIGATION_INCOMPATIBLE_METHODS.contains(&method)
        {
            return;
        }
        let lhs = method_call.receiver().expect("checked above");
        if possible_class_hierarchy_check(method, &lhs, &method_call, ctx) {
            return;
        }

        let message =
            format!("Use `{inverse}` instead of inverting `{}`.", String::from_utf8_lossy(method));

        let mut edits = Vec::new();
        let prefix = Span::new(node.span().start, method_call.as_node().span().start);
        if prefix.start < prefix.end {
            edits.push(Edit::delete(prefix));
        }
        if let Some(msg_loc) = method_call.message_loc() {
            edits.push(Edit::replace(msg_loc.span(), inverse.clone().into_bytes()));
        }
        let is_equality_method = EQUALITY_METHODS.contains(&method);
        if is_equality_method || wrapped_in_parens {
            let tail = Span::new(method_call.as_node().span().end, node.span().end);
            if tail.start < tail.end {
                edits.push(Edit::delete(tail));
            }
        }

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }

    /// Upstream's `on_block`/`on_numblock`/`on_itblock`: a `select`/
    /// `reject`-like call (no arguments of its own) whose block's last
    /// statement negates its value.
    fn check_inverse_block(&mut self, call: &CallNode<'_>, node: &Node<'_>, ctx: &mut Context<'_>) {
        if call.arguments().is_some() {
            return;
        }
        let method = call.name().as_slice();
        let Some(inverse) = self.inverse_blocks.get(method) else { return };
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        let Some(stmts) = block.body().and_then(|b| b.as_statements_node()) else { return };
        let stmt_list = stmts.body();
        let Some(last) = stmt_list.last() else { return };
        let Some(leaf) = last.as_call_node() else { return };
        let leaf_name = leaf.name().as_slice();
        let is_negated_equality = matches!(leaf_name, b"!=" | b"!~");
        if leaf_name != b"!" && !is_negated_equality {
            return;
        }

        if ancestor_is_bang(ctx, 1) && ancestor_is_bang(ctx, 2) {
            return;
        }
        if contains_next(&block.as_node()) {
            return;
        }

        let message =
            format!("Use `{inverse}` instead of inverting `{}`.", String::from_utf8_lossy(method));

        let mut edits = Vec::new();
        if let Some(msg_loc) = call.message_loc() {
            edits.push(Edit::replace(msg_loc.span(), inverse.clone().into_bytes()));
        }
        let Some(sel) = leaf.message_loc().map(|l| l.span()) else { return };
        if is_negated_equality {
            let mut text = ctx.text(sel).to_vec();
            text[0] = b'=';
            edits.push(Edit::replace(sel, text));
        } else if let Some(dot) = leaf.call_operator_loc() {
            // Postfix `BasicObject#!` (`e.bar?.!`): drop the dot through
            // the end of the call, matching upstream's `dot_range`.
            edits.push(Edit::delete(Span::new(dot.span().start, leaf.as_node().span().end)));
        } else {
            edits.push(Edit::delete(sel));
            self.suppressed.insert(leaf.as_node().span().start);
        }

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

impl Rule for InverseMethods {
    const META: RuleMeta = RuleMeta {
        name: "Style/InverseMethods",
        department: Department::Style,
        summary: "Use the inverse method instead of `!.method` if an inverse method is defined.",
        explanation: "\
Checks for usages of not (`not` or `!`) called on a method
when an inverse of that method can be used instead.

Methods that can be inverted by a not (`not` or `!`) should be defined
in `InverseMethods`.

Methods that are inverted by inverting the return
of the block that is passed to the method should be defined in
`InverseBlocks`.

@safety
  This cop is unsafe because it cannot be guaranteed that the method
  and its inverse method are both defined on receiver, and also are
  actually inverse of each other.

```ruby
# bad
!foo.none?
!foo.any? { |f| f.even? }
!foo.blank?
!(foo == bar)
foo.select { |f| !f.even? }
foo.reject { |f| f != 7 }

# good
foo.none?
foo.blank?
foo.any? { |f| f.even? }
foo != bar
foo == bar
!!('foo' =~ /^\\w+$/)
!(foo.class < Numeric) # Checking class hierarchy is allowed
# Blocks with guard clauses are ignored:
foo.select do |f|
  next if f.zero?
  f != 1
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
`InverseMethods`/`InverseBlocks` are read as a plain string-to-string
mapping (no schema default is declared for them, since map-shaped config
values have no `ConfigDefault` representation here); a project overriding
one without the other still gets this cop's real default for the other,
matching upstream's per-key `config/default.yml` merge. Detecting a `!`/
`not` ancestor (the double-negation and guard-clause-block skips) reads
the ancestor's own source text rather than a live parent node, since
`Context::ancestors` carries only kind and span; this is exact for this
shape (see the module docs) rather than an approximation.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            inverse_method_map: bidirectional_map(
                options,
                "InverseMethods",
                DEFAULT_INVERSE_METHODS,
            ),
            inverse_blocks: bidirectional_map(options, "InverseBlocks", DEFAULT_INVERSE_BLOCKS),
            suppressed: std::collections::HashSet::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.suppressed.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() == b"!" {
            self.check_inverse_method(&call, node, ctx);
        } else {
            self.check_inverse_block(&call, node, ctx);
        }
    }
}
