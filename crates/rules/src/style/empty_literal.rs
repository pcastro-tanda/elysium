//! `Style/EmptyLiteral`, ported from RuboCop's
//! `lib/rubocop/cop/style/empty_literal.rb`, together with the parts of the
//! `FrozenStringLiteral`/`StringLiteralsHelp` mixins it needs to decide
//! whether `String.new` is exempt.
//!
//! # Node-pattern shapes
//!
//! Upstream matches four alternatives with `def_node_matcher`:
//! - `array_node`/`hash_node`/`str_node`: `Foo.new` with zero arguments
//!   (`hash_node`/`str_node`), or zero arguments or a single *empty* array
//!   literal argument (`array_node` only -- `Array.new([])`).
//! - `array_with_block`/`hash_with_block`: the same zero-argument `Foo.new`
//!   with any kind of attached block (plain, numbered-parameter, or `it`).
//!   Prism represents all three as one [`ruby_ast::node::BlockNode`]
//!   attached to the same [`ruby_ast::node::CallNode`] via `CallNode::block`
//!   (no separate wrapping node the way whitequark's `block`/`numblock`/
//!   `itblock` are), so this collapses into a single "does this call have a
//!   literal block" check.
//! - `array_with_index`/`hash_with_index`: `Foo[]` (zero arguments), or a
//!   bare `Foo([])`/`Foo []` call (no receiver, method name `Array`/`Hash`,
//!   one argument that is itself an *empty* array literal -- the pattern's
//!   bare `(array)` with no `...` requires exact zero-child arity, matching
//!   rubocop-ast's node-pattern semantics).
//!
//! A `(array)?`/`(array)` element therefore always means "no elements",
//! never "any array": `Array[3]`/`Array [3]`/`Hash[3, 4]` never match.
//!
//! # Rewrapping an unparenthesized first argument
//!
//! `super Hash.new, something` and `yadayada.map { a }.reduce Hash.new` both
//! use `Hash.new` as the unparenthesized first argument of an enclosing
//! `send`/`super`; replacing just `Hash.new` with `{}` there would turn the
//! braces into a block instead of a hash literal, so upstream's
//! `first_argument_unparenthesized?`/`replacement_range`/`correction`
//! rewrite the *whole* enclosing argument list in parentheses instead
//! (`super({}, something)`). Prism has no parent pointers and
//! [`linter::Context::ancestors`] carries only `(span, kind)` pairs, so
//! [`DispatchFacts`] records each `CallNode`/`SuperNode`'s own
//! parenthesization and argument spans as it is entered (mirroring
//! `hash_syntax.rs`'s `AncestorView`/`Facts` cache); when a `Hash.new` node
//! is later visited, its immediate ancestor (skipping the transparent
//! `ArgumentsNode` wrapper) is looked up in that cache to answer the same
//! question. Only `hash_node` (the `Hash.new` shape) is rewrapped, matching
//! upstream's `replacement_range` (which special-cases only `hash_node`,
//! unlike `correction`'s broader `offense_hash_node?` guard); no fixture
//! exercises a `Hash[]`/`Hash(...)` first argument, and mirroring upstream's
//! broader guard there would build a replacement range that does not cover
//! the text `correction` produces.

use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// A node identity: span plus kind, unique enough to key the
/// ancestor-facts cache (mirrors `style/hash_syntax.rs`'s `Key`).
type Key = (Span, NodeKind);

/// A `CallNode`/`SuperNode`'s own parenthesization and argument spans,
/// recorded when the node is entered so a descendant can answer "am I the
/// unparenthesized first argument of my dispatching parent" without a
/// second tree walk.
#[derive(Debug, Clone, Default)]
struct DispatchFacts {
    /// Whether the call/`super` carries real `(...)`.
    parenthesized: bool,
    /// Spans of its own arguments, in source order.
    arg_spans: Vec<Span>,
}

/// Prefer literals to Array.new/Hash.new/String.new.
#[derive(Debug, Clone)]
pub struct EmptyLiteral {
    /// `Style/FrozenStringLiteralComment: Enabled`.
    frozen_string_cop_enabled: bool,
    /// `AllCops: StringLiteralsFrozenByDefault`, tri-state (unset/`true`/`false`).
    string_literals_frozen_by_default: Option<bool>,
    /// `''` or `""`, from `Style/StringLiterals: EnforcedStyle`.
    preferred_string_literal: &'static str,
    /// [`DispatchFacts`] for every `CallNode`/`SuperNode` entered so far.
    dispatch_facts: HashMap<Key, DispatchFacts>,
}

impl Rule for EmptyLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Style/EmptyLiteral",
        department: Department::Style,
        summary: "Prefer literals to Array.new/Hash.new/String.new.",
        explanation: "\
Checks for the use of a method, the result of which would be a
literal, like an empty array, hash, or string.

NOTE: When frozen string literals are enabled, `String.new` isn't
corrected to an empty string since the former is mutable and the
latter would be frozen.

```ruby
# bad
a = Array.new
a = Array[]
h = Hash.new
h = Hash[]
s = String.new

# good
a = []
h = {}
s = ''
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::SuperNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let string_literals_frozen_by_default =
            options.peer("AllCops", "StringLiteralsFrozenByDefault").and_then(OptionValue::as_bool);
        let frozen_string_cop_enabled = options
            .peer("Style/FrozenStringLiteralComment", "Enabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(true);
        let preferred_string_literal =
            if options.peer("Style/StringLiterals", "EnforcedStyle").and_then(OptionValue::as_str)
                == Some("double_quotes")
            {
                "\"\""
            } else {
                "''"
            };
        Ok(Self {
            frozen_string_cop_enabled,
            string_literals_frozen_by_default,
            preferred_string_literal,
            dispatch_facts: HashMap::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                self.check_call(node.span(), &call, ctx);
                self.dispatch_facts.insert(
                    (node.span(), NodeKind::CallNode),
                    DispatchFacts {
                        parenthesized: call.opening_loc().is_some(),
                        arg_spans: call
                            .arguments()
                            .map(|a| a.arguments().iter().map(|n| n.span()).collect())
                            .unwrap_or_default(),
                    },
                );
            }
            NodeKind::SuperNode => {
                let sup = node.as_super_node().expect("kind matched");
                self.dispatch_facts.insert(
                    (node.span(), NodeKind::SuperNode),
                    DispatchFacts {
                        parenthesized: sup.lparen_loc().is_some(),
                        arg_spans: sup
                            .arguments()
                            .map(|a| a.arguments().iter().map(|n| n.span()).collect())
                            .unwrap_or_default(),
                    },
                );
            }
            _ => {}
        }
    }
}

impl EmptyLiteral {
    /// RuboCop's `on_send` plus `offense_message`/`replacement_range`/`correction`.
    fn check_call(&mut self, span: Span, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let is_hash_new = is_new_call(call, "Hash", false) && !has_literal_block(call);
        let is_hash_offense = is_hash_new
            || matches_bracket_index(call, "Hash")
            || matches_call_with_empty_array(call, "Hash");
        let is_array_offense = (is_new_call(call, "Array", true) && !has_literal_block(call))
            || matches_bracket_index(call, "Array")
            || matches_call_with_empty_array(call, "Array");

        if is_array_offense {
            let current = String::from_utf8_lossy(ctx.text(span)).into_owned();
            let message = format!("Use array literal `[]` instead of `{current}`.");
            let fix = Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, b"[]".as_slice())],
            };
            ctx.report_with_fix(&Self::META, span, message, fix);
            return;
        }

        if is_hash_offense {
            let current = String::from_utf8_lossy(ctx.text(span)).into_owned();
            let message = format!("Use hash literal `{{}}` instead of `{current}`.");
            let wrap =
                is_hash_new.then(|| self.unparenthesized_first_arg_facts(span, ctx)).flatten();
            let (fix_span, replacement) = match wrap {
                Some(facts) => build_wrap(ctx, facts),
                None => (span, b"{}".to_vec()),
            };
            let fix = Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(fix_span, replacement)],
            };
            ctx.report_with_fix(&Self::META, span, message, fix);
            return;
        }

        if is_new_call(call, "String", false) && !self.frozen_strings(ctx) {
            let message = format!(
                "Use string literal `{}` instead of `String.new`.",
                self.preferred_string_literal
            );
            let fix = Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, self.preferred_string_literal.as_bytes().to_vec())],
            };
            ctx.report_with_fix(&Self::META, span, message, fix);
        }
    }

    /// The immediate dispatching ancestor's [`DispatchFacts`], if `span` is
    /// exactly its unparenthesized first argument. RuboCop's
    /// `first_argument_unparenthesized?`: `node.parent` is a `send`/`super`
    /// (skipping Prism's transparent `ArgumentsNode` wrapper, which
    /// whitequark has no equivalent of) whose `first_argument` is `node`
    /// itself and which carries no real parentheses.
    fn unparenthesized_first_arg_facts(
        &self,
        span: Span,
        ctx: &Context<'_>,
    ) -> Option<&DispatchFacts> {
        let ancestors = ctx.ancestors();
        let mut idx = ancestors.len().checked_sub(1)?;
        if ancestors[idx].kind == NodeKind::ArgumentsNode {
            idx = idx.checked_sub(1)?;
        }
        let info = ancestors[idx];
        if !matches!(info.kind, NodeKind::CallNode | NodeKind::SuperNode) {
            return None;
        }
        let facts = self.dispatch_facts.get(&(info.span, info.kind))?;
        (!facts.parenthesized && facts.arg_spans.first() == Some(&span)).then_some(facts)
    }

    /// RuboCop's `frozen_strings?`.
    fn frozen_strings(&self, ctx: &Context<'_>) -> bool {
        if let Some(specified) = magic_frozen_string_literal(ctx) {
            return specified;
        }
        match self.string_literals_frozen_by_default {
            Some(default) => default,
            None => self.frozen_string_cop_enabled,
        }
    }
}

/// RuboCop's `frozen_string_literals_enabled?`'s magic-comment lookup: the
/// leading `# frozen_string_literal: <value>` (or `frozen-string-literal`,
/// case-insensitively on both the key and a `true`/`false` value) comment
/// Prism itself recognized, if any. `None` when no such comment is present.
fn magic_frozen_string_literal(ctx: &Context<'_>) -> Option<bool> {
    ctx.parsed().magic_comments().find_map(|comment| {
        let key = comment.key();
        (key.eq_ignore_ascii_case(b"frozen_string_literal")
            || key.eq_ignore_ascii_case(b"frozen-string-literal"))
        .then(|| comment.value().eq_ignore_ascii_case(b"true"))
    })
}

/// RuboCop's `array_node`/`hash_node`/`str_node`: `(send (const {nil? cbase}
/// name) :new)`, optionally also matching a single empty-array-literal
/// argument when `allow_empty_array_arg` (only true for `array_node`).
fn is_new_call(call: &CallNode<'_>, const_name: &str, allow_empty_array_arg: bool) -> bool {
    if call.is_safe_navigation() || call.name().as_slice() != b"new" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    if !ext::is_bare_or_toplevel_const(&receiver)
        || ext::const_name(&receiver).as_deref() != Some(const_name)
    {
        return false;
    }
    match call.arguments() {
        None => true,
        Some(args) => {
            let list = args.arguments();
            if list.is_empty() {
                true
            } else {
                allow_empty_array_arg
                    && list.len() == 1
                    && is_empty_array(&list.first().expect("checked len"))
            }
        }
    }
}

/// RuboCop's `array_with_block`/`hash_with_block`: any literal block
/// (plain, numbered-parameter, or `it`) attached to the call. A `&block`
/// argument pass is a `BlockArgumentNode`, not a literal block, and does
/// not count.
fn has_literal_block(call: &CallNode<'_>) -> bool {
    call.block().is_some_and(|b| b.as_block_node().is_some())
}

/// RuboCop's `array_with_index`/`hash_with_index` first alternative:
/// `(send (const {nil? cbase} name) :[])`, zero arguments.
fn matches_bracket_index(call: &CallNode<'_>, const_name: &str) -> bool {
    if call.is_safe_navigation() || call.name().as_slice() != b"[]" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    ext::is_bare_or_toplevel_const(&receiver)
        && ext::const_name(&receiver).as_deref() == Some(const_name)
        && call.arguments().is_none_or(|a| a.arguments().is_empty())
}

/// RuboCop's `array_with_index`/`hash_with_index` second alternative:
/// `(send nil? name (array))` -- a bare `Array([])`/`Hash([])`/`Array
/// []`/`Hash [3]`-shaped call whose sole argument is an *empty* array
/// literal (the pattern's bare `(array)` requires exact zero-child arity).
fn matches_call_with_empty_array(call: &CallNode<'_>, const_name: &str) -> bool {
    if call.is_safe_navigation()
        || call.receiver().is_some()
        || call.name().as_slice() != const_name.as_bytes()
    {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let list = args.arguments();
    list.len() == 1 && is_empty_array(&list.first().expect("checked len"))
}

/// An empty `[...]` array literal.
fn is_empty_array(node: &Node<'_>) -> bool {
    node.as_array_node().is_some_and(|a| a.elements().is_empty())
}

/// RuboCop's `replacement_range`/`correction` for a `hash_node` match that
/// is its dispatching parent's unparenthesized first argument: replaces
/// from one byte before the first argument (consuming the separating
/// space) through the last argument's end, with `(` + `{}` + every other
/// argument's own source, comma-joined, + `)`.
fn build_wrap(ctx: &Context<'_>, facts: &DispatchFacts) -> (Span, Vec<u8>) {
    let first = facts.arg_spans[0];
    let last = *facts.arg_spans.last().expect("first exists");
    let range = Span::new(first.start - 1, last.end);
    let mut replacement = Vec::with_capacity(4);
    replacement.push(b'(');
    replacement.extend_from_slice(b"{}");
    for arg in &facts.arg_spans[1..] {
        replacement.extend_from_slice(b", ");
        replacement.extend_from_slice(ctx.text(*arg));
    }
    replacement.push(b')');
    (range, replacement)
}
