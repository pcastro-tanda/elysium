//! `Style/MutableConstant`, ported from RuboCop's
//! `lib/rubocop/cop/style/mutable_constant.rb`.
//!
//! RuboCop's `on_casgn` only inspects plain `CONST = value` and
//! `CONST ||= value` assignments (an op-asgn target `casgn` node with a
//! non-`or_asgn` parent returns early); `+=`, `-=`, `&&=`, and multiple
//! assignment (`A, B = ...`) are never checked, and neither are we. Prism
//! splits those two forms, plus their `A::B::CONST` path variants, into four
//! node kinds: [`NodeKind::ConstantWriteNode`],
//! [`NodeKind::ConstantPathWriteNode`], [`NodeKind::ConstantOrWriteNode`],
//! [`NodeKind::ConstantPathOrWriteNode`].
//!
//! `shareable_constant_value` handling is delegated entirely to Prism: when
//! a `# shareable_constant_value: <value>` magic comment is in scope for a
//! constant write, Prism wraps that write node in a
//! [`NodeKind::ShareableConstantNode`], which we detect by matching the
//! wrapped write's span rather than re-scanning comments the way RuboCop's
//! (pre-Prism) `ShareableConstantValue` mixin does.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Freeze mutable objects assigned to constants.";

/// `str`, `dstr`, `xstr`, `array`, `hash`, `regexp`, `irange`, `erange` minus
/// the regexp/range literals RuboCop treats as always frozen since Ruby 3.0
/// (`frozen_regexp_or_range_literals?`; see the module blind spot below).
fn is_mutable_literal(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
    )
}

/// RuboCop's `IMMUTABLE_LITERALS`, plus `regexp`/`range` (always frozen
/// under our Ruby >= 3.0 assumption).
fn is_immutable_literal(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::RangeNode
    )
}

/// RuboCop's `uninterpolated_string?`: a plain string, or a concatenation of
/// string parts with no `#{}`/`#@ivar`-style interpolation. Concatenated
/// segments that themselves contain interpolation parse as a nested
/// `InterpolatedStringNode` (not a flat `StringNode` part), so this
/// recurses rather than only checking one level of parts.
fn is_uninterpolated_string(value: &Node<'_>) -> bool {
    match value.kind() {
        NodeKind::StringNode => true,
        NodeKind::InterpolatedStringNode => value
            .as_interpolated_string_node()
            .expect("kind matched")
            .parts()
            .iter()
            .all(|part| is_uninterpolated_string(&part)),
        _ => false,
    }
}

/// RuboCop's `frozen_string_literal?`: an uninterpolated string/heredoc is
/// exempt when `# frozen_string_literal: true` is in effect for the file.
fn is_frozen_string_literal(value: &Node<'_>, ctx: &Context<'_>) -> bool {
    is_uninterpolated_string(value) && ctx.parsed().frozen_string_literals()
}

/// A bare `Name` or top-level `::Name` constant reference (RuboCop's
/// `(const {nil? cbase} :Name)`). Shape check delegated to
/// [`ruby_ast::ext::is_bare_or_toplevel_const`]; the name comparison stays
/// local since the shared helper only checks shape.
fn is_bare_or_toplevel_const(node: &Node<'_>, expected: &[u8]) -> bool {
    if !ext::is_bare_or_toplevel_const(node) {
        return false;
    }
    match node.kind() {
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().is_some_and(|n| n.name().as_slice() == expected)
        }
        NodeKind::ConstantPathNode => node
            .as_constant_path_node()
            .and_then(|path| path.name())
            .is_some_and(|id| id.as_slice() == expected),
        _ => unreachable!("ext::is_bare_or_toplevel_const already checked the shape"),
    }
}

/// The sole argument of a call, if it has exactly one.
fn only_argument<'pr>(call: &ruby_ast::node::CallNode<'pr>) -> Option<Node<'pr>> {
    let args = call.arguments()?;
    let list = args.arguments();
    if list.len() != 1 {
        return None;
    }
    list.iter().next()
}

/// `ENV[...]` or `::ENV[...]` (RuboCop's `(send (const {nil? cbase} :ENV) :[] _)`).
fn is_env_index(call: &ruby_ast::node::CallNode<'_>) -> bool {
    call.name().as_slice() == b"[]"
        && call.receiver().is_some_and(|r| is_bare_or_toplevel_const(&r, b"ENV"))
}

/// RuboCop's `operation_produces_immutable_object?`: patterns the cop
/// considers frozen-safe in `strict` mode even though they are not literals.
fn produces_immutable_object(value: &Node<'_>) -> bool {
    match value.kind() {
        NodeKind::ConstantReadNode | NodeKind::ConstantPathNode => true,
        NodeKind::CallNode => {
            let call = value.as_call_node().expect("kind matched");
            match call.name().as_slice() {
                b"new" => call.receiver().is_some_and(|r| is_bare_or_toplevel_const(&r, b"Struct")),
                b"freeze" | b"count" | b"length" | b"size" | b"==" | b"===" | b"!=" | b"<="
                | b">=" | b"<" | b">" => true,
                b"+" | b"-" | b"*" | b"**" | b"/" | b"%" => {
                    let receiver_is_number = call.receiver().is_some_and(|r| {
                        matches!(r.kind(), NodeKind::IntegerNode | NodeKind::FloatNode)
                    });
                    let arg_is_number = only_argument(&call).is_some_and(|a| {
                        matches!(a.kind(), NodeKind::IntegerNode | NodeKind::FloatNode)
                    });
                    receiver_is_number || arg_is_number
                }
                _ => is_env_index(&call),
            }
        }
        NodeKind::OrNode => {
            let or_node = value.as_or_node().expect("kind matched");
            let left = or_node.left();
            left.kind() == NodeKind::CallNode
                && is_env_index(&left.as_call_node().expect("kind matched"))
        }
        _ => false,
    }
}

/// RuboCop's `splat_value` node matcher: `(array (splat $_))`.
fn splat_value<'pr>(value: &Node<'pr>) -> Option<Node<'pr>> {
    if value.kind() != NodeKind::ArrayNode {
        return None;
    }
    let elements = value.as_array_node().expect("kind matched").elements();
    if elements.len() != 1 {
        return None;
    }
    let only = elements.iter().next()?;
    if only.kind() != NodeKind::SplatNode {
        return None;
    }
    only.as_splat_node().expect("kind matched").expression()
}

/// RuboCop's `range_enclosed_in_parentheses?`: `(begin (range _ _))`.
fn is_range_enclosed_in_parentheses(node: &Node<'_>) -> bool {
    if node.kind() != NodeKind::ParenthesesNode {
        return false;
    }
    let mut body = node.as_parentheses_node().expect("kind matched").body();
    if let Some(inner) = &body {
        if inner.kind() == NodeKind::StatementsNode {
            let stmts = inner.as_statements_node().expect("kind matched").body();
            body = if stmts.len() == 1 { stmts.iter().next() } else { None };
        }
    }
    body.is_some_and(|b| b.kind() == NodeKind::RangeNode)
}

/// RuboCop's `requires_parentheses?`: `node.range_type? || (node.send_type?
/// && node.loc.dot.nil?)`.
fn requires_parentheses(value: &Node<'_>) -> bool {
    match value.kind() {
        NodeKind::RangeNode => true,
        NodeKind::CallNode => {
            value.as_call_node().expect("kind matched").call_operator_loc().is_none()
        }
        _ => false,
    }
}

/// The span RuboCop's `node.source_range` would report for `value`. Matches
/// [`Node::span`] for everything except a heredoc, whose source range in
/// RuboCop is only the opening `<<~TAG` token (the body/terminator are
/// tracked separately), not the full multi-line construct Prism's own
/// location covers.
fn value_span(value: &Node<'_>, ctx: &Context<'_>) -> Span {
    let opening = match value.kind() {
        NodeKind::StringNode => value.as_string_node().expect("kind matched").opening_loc(),
        NodeKind::InterpolatedStringNode => {
            value.as_interpolated_string_node().expect("kind matched").opening_loc()
        }
        _ => None,
    };
    if let Some(opening) = opening {
        let span = opening.span();
        if ctx.text(span).starts_with(b"<<") {
            return span;
        }
    }
    value.span()
}

/// RuboCop's `autocorrect`: wraps `value` as needed, then always appends
/// `.freeze` right after it.
fn build_fix(value: &Node<'_>, ctx: &Context<'_>) -> Fix {
    let expr = value_span(value, ctx);
    let mut edits = Vec::with_capacity(3);

    if let Some(splat) = splat_value(value) {
        let source = ctx.text(splat.span());
        let replacement = if is_range_enclosed_in_parentheses(&splat) {
            format!("{}.to_a", String::from_utf8_lossy(source))
        } else {
            format!("({}).to_a", String::from_utf8_lossy(source))
        };
        edits.push(Edit::replace(expr, replacement.into_bytes()));
    } else if value.kind() == NodeKind::ArrayNode
        && value.as_array_node().expect("kind matched").opening_loc().is_none()
    {
        edits.push(Edit::insert(expr.start, b"[".to_vec()));
        edits.push(Edit::insert(expr.end, b"]".to_vec()));
    } else if requires_parentheses(value) {
        edits.push(Edit::insert(expr.start, b"(".to_vec()));
        edits.push(Edit::insert(expr.end, b")".to_vec()));
    }
    edits.push(Edit::insert(expr.end, b".freeze".to_vec()));

    Fix { applicability: Applicability::Unsafe, edits }
}

/// Checks whether `EnforcedStyle: literals` allows `value`.
fn literals_check(value: &Node<'_>, ctx: &Context<'_>) -> bool {
    is_mutable_literal(value.kind()) && !is_frozen_string_literal(value, ctx)
}

/// Checks whether `EnforcedStyle: strict` allows `value`.
fn strict_check(value: &Node<'_>, ctx: &Context<'_>) -> bool {
    if is_immutable_literal(value.kind()) {
        return false;
    }
    if produces_immutable_object(value) {
        return false;
    }
    !is_frozen_string_literal(value, ctx)
}

/// Style the cop enforces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnforcedStyle {
    Literals,
    Strict,
}

/// Looks for mutable literals (or, in `strict` mode, anything not known to
/// be frozen) assigned to constants and suggests freezing them.
#[derive(Debug, Clone)]
pub struct MutableConstant {
    style: EnforcedStyle,
    /// Spans of writes that a [`NodeKind::ShareableConstantNode`] wrapped:
    /// the parser has already determined a `shareable_constant_value` magic
    /// comment applies to them, so they are exempt.
    shareable_writes: Vec<Span>,
}

impl MutableConstant {
    fn is_shareable(&mut self, node: &Node<'_>) -> bool {
        let span = node.span();
        if let Some(pos) = self.shareable_writes.iter().position(|s| *s == span) {
            self.shareable_writes.remove(pos);
            true
        } else {
            false
        }
    }

    fn on_assignment(&mut self, node: &Node<'_>, value: &Node<'_>, ctx: &mut Context<'_>) {
        if self.is_shareable(node) {
            return;
        }
        let offense = match self.style {
            EnforcedStyle::Literals => literals_check(value, ctx),
            EnforcedStyle::Strict => strict_check(value, ctx),
        };
        if !offense {
            return;
        }
        let span = value_span(value, ctx);
        let fix = build_fix(value, ctx);
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}

impl Rule for MutableConstant {
    const META: RuleMeta = RuleMeta {
        name: "Style/MutableConstant",
        department: Department::Style,
        summary: "Do not assign mutable objects to constants.",
        explanation: "\
Checks whether some constant value isn't a mutable literal (e.g. array or
hash).

```ruby
# bad
CONST = [1, 2, 3]

# good
CONST = [1, 2, 3].freeze
```

Strict mode (`EnforcedStyle: strict`) freezes all constants, not just
literals:

```ruby
# bad (strict mode only)
CONST = Something.new

# good
CONST = Something.new.freeze
```

Strict mode is considered experimental: it does not have an exhaustive list
of methods that produce frozen objects, so it has a decent chance of false
positives. There is no harm in freezing an already frozen object, though.

`Regexp` and `Range` literals have been frozen since Ruby 3.0 and are never
flagged. A `# shareable_constant_value: literal` (or `experimental_everything`
/`experimental_copy`) magic comment also suppresses offenses for the
constant writes it covers, matching Ruby's own Ractor shareable-constant
semantics.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::ConstantOrWriteNode,
            NodeKind::ConstantPathOrWriteNode,
            NodeKind::ShareableConstantNode,
        ],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("literals"),
            allowed: &["literals", "strict"],
            doc: "`literals` freezes only literal values assigned to constants; \
`strict` freezes every constant assignment.",
        }],
        blind_spots: "\
Only plain `CONST = value` and `CONST ||= value` (and their `A::B::CONST`
path forms) are checked, matching RuboCop's own `on_casgn`: `+=`, `-=`,
`&&=`, other compound assignments, and multiple assignment (`A, B = x, y`)
are never inspected by RuboCop either, since their `casgn` target node's
parent is not an `or_asgn` node.

`TargetRubyVersion` is not modeled: this rule always assumes Ruby >= 3.0
semantics (`Regexp`/`Range` literals frozen, `frozen_string_literal` honored
per Ruby 3.0 rules). RuboCop's own Ruby <= 2.7 branches are unreachable when
parsing with Prism (they are tagged `unsupported_on: :prism` in RuboCop's
own spec suite), so this matches real behavior for every file this engine
can parse.

`shareable_constant_value` is read from Prism's native
`ShareableConstantNode` wrapping rather than re-scanning magic comments by
hand; this defers entirely to Prism's own (spec-verified) scoping instead of
reimplementing it.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "strict" => EnforcedStyle::Strict,
            _ => EnforcedStyle::Literals,
        };
        Ok(Self { style, shareable_writes: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.shareable_writes.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ShareableConstantNode => {
                let write = node.as_shareable_constant_node().expect("kind matched").write();
                self.shareable_writes.push(write.span());
            }
            NodeKind::ConstantWriteNode => {
                let value = node.as_constant_write_node().expect("kind matched").value();
                self.on_assignment(node, &value, ctx);
            }
            NodeKind::ConstantPathWriteNode => {
                let value = node.as_constant_path_write_node().expect("kind matched").value();
                self.on_assignment(node, &value, ctx);
            }
            NodeKind::ConstantOrWriteNode => {
                let value = node.as_constant_or_write_node().expect("kind matched").value();
                self.on_assignment(node, &value, ctx);
            }
            NodeKind::ConstantPathOrWriteNode => {
                let value = node.as_constant_path_or_write_node().expect("kind matched").value();
                self.on_assignment(node, &value, ctx);
            }
            _ => {}
        }
    }
}
