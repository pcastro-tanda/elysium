//! `Style/RedundantFreeze`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_freeze.rb`, together with the parts of
//! its `FrozenStringLiteral` mixin and rubocop-ast's `Node#immutable_literal?`
//! needed to decide whether `.freeze` has any effect.
//!
//! # `frozen_string_literal?`
//!
//! Upstream's `FrozenStringLiteral#frozen_string_literals_enabled?` checks,
//! in order: an explicit `# frozen_string_literal: true`/`false` magic
//! comment (wins outright, either way), else `AllCops:
//! StringLiteralsFrozenByDefault`, else `false`. Prism's own
//! `ParseResult::frozen_string_literals` only reports the first half (true
//! when an explicit `true` comment is in effect), so it cannot tell "no
//! comment, default true" apart from "explicit `false`, default true" --
//! [`magic_frozen_string_literal`] reads the comment key/value directly from
//! `Context::parsed().magic_comments()` to recover that distinction, and
//! only falls back to the configured default when no comment specifies the
//! setting at all.
//!
//! # `operation_produces_immutable_object?`
//!
//! Upstream's node pattern is five alternatives; ported as
//! [`produces_immutable_object`]. The `count`/`length`/`size` alternatives
//! (whitequark's bare `send` and `any_block`-wrapped `send`) collapse into
//! one check here, since Prism represents both a blockless and a
//! block-attached call as the same `CallNode` (the block simply is or is
//! not present in `CallNode::block`) -- there is no separate wrapping node
//! kind to match against, unlike whitequark's `:block`/`:numblock`/
//! `:itblock`. The remaining three alternatives all require the whitequark
//! `begin` wrapper (parenthesization), which is Prism's `ParenthesesNode`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Do not freeze immutable objects, as freezing them has no effect.";

/// Checks usages of `Object#freeze` on immutable objects.
#[derive(Debug, Clone)]
pub struct RedundantFreeze {
    /// `AllCops: TargetRubyVersion`.
    target_ruby_version: f32,
    /// `AllCops: StringLiteralsFrozenByDefault`.
    string_literals_frozen_by_default: bool,
}

impl Rule for RedundantFreeze {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantFreeze",
        department: Department::Style,
        summary: "Checks usages of Object#freeze on immutable objects.",
        explanation: "\
Checks for uses of `Object#freeze` on immutable objects.

NOTE: `Regexp` and `Range` literals are frozen objects since Ruby 3.0.

NOTE: From Ruby 3.0, this cop allows explicit freezing of interpolated
string literals when `# frozen-string-literal: true` is used.

```ruby
# bad
CONST = 1.freeze

# good
CONST = 1
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let string_literals_frozen_by_default = options
            .peer("AllCops", "StringLiteralsFrozenByDefault")
            .and_then(OptionValue::as_bool)
            .unwrap_or(false);
        Ok(Self {
            target_ruby_version: options.target_ruby_version(),
            string_literals_frozen_by_default,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() || call.name().as_slice() != b"freeze" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let frozen = is_immutable_literal(strip_parens(receiver), self.target_ruby_version)
            || is_frozen_string_literal(
                strip_parens(receiver),
                ctx,
                self.string_literals_frozen_by_default,
            )
            || produces_immutable_object(&receiver);
        if !frozen {
            return;
        }
        let span = call.as_node().span();
        match build_fix(&call) {
            Some(fix) => ctx.report_with_fix(&Self::META, span, MSG, fix),
            None => ctx.report(&Self::META, span, MSG),
        }
    }
}

/// RuboCop's `strip_parenthesis`: unwraps a single level of `(...)` around a
/// lone expression (Prism's `ParenthesesNode` with a one-statement body); an
/// empty `()`, or anything else, passes through unchanged.
fn strip_parens(node: Node<'_>) -> Node<'_> {
    single_statement(&node).unwrap_or(node)
}

/// The sole statement of a `ParenthesesNode`'s body, if `node` is one and
/// its body holds exactly one statement.
fn single_statement<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let body = node.as_parentheses_node()?.body()?;
    let stmts = body.as_statements_node()?;
    (stmts.body().len() == 1).then(|| stmts.body().first()).flatten()
}

/// rubocop-ast's `Node#immutable_literal?`: `IMMUTABLE_LITERALS.include?(type)`
/// (integers, floats, symbols, `true`/`false`/`nil`, complex and rational
/// literals), plus upstream's own `target_ruby_version >= 3.0 &&
/// node.type?(:regexp, :range)` fallback (regex/range literals are frozen
/// objects since Ruby 3.0). `frozen_string_literal?` is checked separately
/// by the caller, against the *unstripped* node's parenthesization already
/// resolved.
fn is_immutable_literal(node: Node<'_>, target_ruby_version: f32) -> bool {
    if matches!(
        node.kind(),
        NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
    ) {
        return true;
    }

    target_ruby_version >= 3.0
        && matches!(node.kind(), NodeKind::RangeNode | NodeKind::RegularExpressionNode)
}

/// RuboCop's `uninterpolated_string?`: a plain string, or a concatenation of
/// string parts with no `#{}`/`#@ivar`-style interpolation. Concatenated
/// segments that themselves contain interpolation parse as a nested
/// `InterpolatedStringNode` (not a flat `StringNode` part), so this recurses
/// rather than only checking one level of parts.
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

/// RuboCop's `FrozenStringLiteral#frozen_string_literal?`, restricted to our
/// `target_ruby_version >= 3.0` assumption (fixtures default to 3.3): an
/// uninterpolated string/heredoc is exempt when frozen string literals are
/// enabled for the file.
fn is_frozen_string_literal(
    value: Node<'_>,
    ctx: &Context<'_>,
    string_literals_frozen_by_default: bool,
) -> bool {
    is_uninterpolated_string(&value)
        && magic_frozen_string_literal(ctx).unwrap_or(string_literals_frozen_by_default)
}

/// RuboCop's `frozen_string_literals_enabled?`'s magic-comment lookup: the
/// leading `# frozen_string_literal: <value>` (or `frozen-string-literal`,
/// case-insensitively on both the key and a `true`/`false` value) comment
/// Prism itself recognized, if any. `None` when no such comment is present,
/// in which case the caller falls back to `AllCops:
/// StringLiteralsFrozenByDefault`.
fn magic_frozen_string_literal(ctx: &Context<'_>) -> Option<bool> {
    ctx.parsed().magic_comments().find_map(|comment| {
        let key = comment.key();
        (key.eq_ignore_ascii_case(b"frozen_string_literal")
            || key.eq_ignore_ascii_case(b"frozen-string-literal"))
        .then(|| comment.value().eq_ignore_ascii_case(b"true"))
    })
}

/// RuboCop's `operation_produces_immutable_object?` node matcher.
fn produces_immutable_object(node: &Node<'_>) -> bool {
    if is_count_length_size_call(node) {
        return true;
    }
    let Some(call) = single_statement(node).and_then(|inner| inner.as_call_node()) else {
        return false;
    };
    let Some(op_receiver) = call.receiver() else { return false };
    let Some(arg) = single_argument(&call) else { return false };
    let op = call.name();
    let is_int_or_float =
        |n: &Node<'_>| matches!(n.kind(), NodeKind::IntegerNode | NodeKind::FloatNode);
    match op.as_slice() {
        b"+" | b"-" | b"*" | b"**" | b"/" | b"%" | b"<<" if is_int_or_float(&op_receiver) => true,
        b"+" | b"-" | b"*" | b"**" | b"/" | b"%"
            if !matches!(op_receiver.kind(), NodeKind::StringNode | NodeKind::ArrayNode)
                && is_int_or_float(&arg) =>
        {
            true
        }
        b"==" | b"===" | b"!=" | b"<=" | b">=" | b"<" | b">" => true,
        _ => false,
    }
}

/// `(send _ {:count :length :size} ...)` / `(any_block (send _ {...} ...) ...)`:
/// a `count`/`length`/`size` call, with or without an attached block (Prism
/// represents both shapes as the same `CallNode`).
fn is_count_length_size_call(node: &Node<'_>) -> bool {
    node.as_call_node()
        .is_some_and(|call| matches!(call.name().as_slice(), b"count" | b"length" | b"size"))
}

/// The sole argument of a call, if it has exactly one.
fn single_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let args = call.arguments()?.arguments();
    (args.len() == 1).then(|| args.first()).flatten()
}

/// RuboCop's autocorrect: `corrector.remove(node.loc.dot);
/// corrector.remove(node.loc.selector)` -- deletes the `.freeze` from the
/// call operator through the method name, keeping everything else.
fn build_fix(call: &CallNode<'_>) -> Option<Fix> {
    let dot = call.call_operator_loc()?;
    let selector = call.message_loc()?;
    let span = Span::new(dot.span().start, selector.span().end);
    Some(Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] })
}
