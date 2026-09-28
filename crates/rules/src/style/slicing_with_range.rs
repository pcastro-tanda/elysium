//! `Style/SlicingWithRange`, ported from RuboCop's
//! `lib/rubocop/cop/style/slicing_with_range.rb`.
//!
//! # Node-pattern `nil` traps
//!
//! Upstream matches range endpoints with three whitequark node patterns
//! whose bare `nil` element is easy to misread:
//!
//! ```text
//! (irange (int 0) {(int -1) nil})   ; useless: ary[0..-1], ary[0..nil]
//! (erange (int 0) nil)              ; useless: ary[0...nil]
//! (irange !nil? {(int -1) nil})     ; endless:  ary[1..-1], ary[1..nil]
//! (erange !nil? nil)                ; endless:  ary[1...nil]
//! (irange nil !nil?)                ; beginless: ary[nil..42]
//! ```
//!
//! A bare `nil` pattern element matches only an *explicit* `(nil)` node
//! (the `nil` keyword written out, e.g. `1..nil`); it does **not** match an
//! omitted endpoint (e.g. `1..`, whose right child is Ruby's actual `nil`,
//! not a node). Verified directly against `RuboCop::AST::NodePattern`: the
//! pattern `(irange !nil? {(int -1) nil})` matches `ary[1..-1]` and
//! `ary[1..nil]` but not `ary[1..]` -- otherwise the cop would loop forever
//! "fixing" already-idiomatic endless ranges. Prism's `RangeNode` already
//! distinguishes `right() == None` (omitted) from `right() ==
//! Some(NilNode)` (explicit `nil`), so this falls out naturally: `!nil?`
//! becomes "present and not a `NilNode`", and bare `nil` becomes "present
//! and is a `NilNode`".
//!
//! # `minimum_target_ruby_version 2.6` not ported
//!
//! Upstream disables the whole cop below Ruby 2.6 via `TargetRubyVersion`;
//! the one spec context exercising that (`ary[1..-1]` on `<= Ruby 2.5`) is
//! itself `unsupported_on: :prism`, so there is nothing to port it against
//! here (same rationale as `layout/heredoc_indentation.rs`).
//!
//! `target_ruby_version >= 2.7` *is* ported: it gates the beginless-range
//! rewrite (`ary[nil..42]` -> `ary[..42]`), which the fixtures exercise
//! directly at the default target version (3.3).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, RangeNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Upstream `MSG`.
const MSG_TEMPLATE: &str = "Prefer `{prefer}` over `{current}`.";
/// Upstream `MSG_USELESS_RANGE`.
const MSG_USELESS_TEMPLATE: &str = "Remove the useless `{prefer}`.";

/// Checks array slicing is done with redundant, endless, and beginless ranges when suitable.
#[derive(Debug, Clone)]
pub struct SlicingWithRange {
    target_ruby_version: f32,
}

impl Rule for SlicingWithRange {
    const META: RuleMeta = RuleMeta {
        name: "Style/SlicingWithRange",
        department: Department::Style,
        summary: "Checks array slicing is done with redundant, endless, and beginless ranges when suitable.",
        explanation: "\
Checks that arrays are not sliced with the redundant `ary[0..-1]`, replacing it with `ary`, \
and ensures arrays are sliced with endless ranges instead of `ary[start..-1]` on Ruby 2.6+, \
and with beginless ranges instead of `ary[nil..end]` on Ruby 2.7+.

This cop is unsafe because `x..-1` and `x..` are only guaranteed to be equivalent for \
`Array#[]`, `String#[]`, and the cop cannot determine what class the receiver is.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Upstream's `minimum_target_ruby_version 2.6` (disabling the whole cop pre-2.6) is not ported: \
the one spec scenario for it (`ary[1..-1]` reporting no offense on Ruby 2.5) is itself marked \
`unsupported_on: :prism`, matching `layout/heredoc_indentation.rs`'s precedent.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"[]" {
            return;
        }
        let Some(args) = call.arguments() else { return };
        if args.arguments().len() != 1 {
            return;
        }
        let Some(range_node) = args.arguments().iter().next().and_then(|n| n.as_range_node())
        else {
            return;
        };

        let offense_range = find_offense_range(&call);
        let Some((message, removal_range)) = offense_message_with_removal_range(
            self.target_ruby_version,
            &call,
            &range_node,
            offense_range,
            ctx,
        ) else {
            return;
        };

        // Changing the range to beginningless or endless when unparenthesized
        // changes the semantics of the code, and thus will not be considered
        // an offense.
        if removal_range != offense_range && unparenthesized_call(&call) {
            return;
        }

        ctx.report_with_fix(
            &Self::META,
            offense_range,
            message,
            Fix { applicability: Applicability::Unsafe, edits: vec![Edit::delete(removal_range)] },
        );
    }
}

/// `unparenthesized_call?`: a dot call (`ary.[] 1..-1`) with no parens
/// around its arguments.
fn unparenthesized_call(call: &CallNode<'_>) -> bool {
    call.call_operator_loc().is_some() && call.opening_loc().is_none()
}

/// `find_offense_range`: the dot-to-end-of-call span for a dot call
/// (`.[](1..-1)`/`&.[](1..-1)`), or the `[...]` selector span for bracket
/// syntax (`[1..-1]`).
fn find_offense_range(call: &CallNode<'_>) -> Span {
    if let Some(dot) = call.call_operator_loc() {
        Span::new(dot.span().start, call.as_node().span().end)
    } else {
        call.message_loc().expect("`[]` call always has a selector").span()
    }
}

/// A range endpoint that is present and is an explicit `nil` literal
/// (whitequark's bare `nil` pattern element -- never matches an omitted
/// endpoint, see the module doc).
fn is_explicit_nil(endpoint: Option<&Node<'_>>) -> bool {
    endpoint.is_some_and(|n| n.kind() == NodeKind::NilNode)
}

/// A range endpoint that is present and is *not* an explicit `nil` literal
/// (whitequark's `!nil?` pattern element).
fn is_present_non_nil(endpoint: Option<&Node<'_>>) -> bool {
    endpoint.is_some_and(|n| n.kind() != NodeKind::NilNode)
}

/// A range endpoint that is the integer literal `value`.
fn is_integer(endpoint: Option<&Node<'_>>, value: i32) -> bool {
    endpoint
        .and_then(Node::as_integer_node)
        .is_some_and(|n| TryInto::<i32>::try_into(n.value()) == Ok(value))
}

/// `range_from_zero_till_minus_one?`: `(irange (int 0) {(int -1) nil})` or
/// `(erange (int 0) nil)` -- a full-array slice, e.g. `ary[0..-1]`.
fn range_from_zero_till_minus_one(range: &RangeNode<'_>) -> bool {
    let left = range.left();
    let right = range.right();
    if range.is_exclude_end() {
        is_integer(left.as_ref(), 0) && is_explicit_nil(right.as_ref())
    } else {
        is_integer(left.as_ref(), 0)
            && (is_integer(right.as_ref(), -1) || is_explicit_nil(right.as_ref()))
    }
}

/// `range_till_minus_one?`: `(irange !nil? {(int -1) nil})` or `(erange
/// !nil? nil)` -- a slice to the end that can become an endless range,
/// e.g. `ary[1..-1]`.
fn range_till_minus_one(range: &RangeNode<'_>) -> bool {
    let left = range.left();
    let right = range.right();
    if range.is_exclude_end() {
        is_present_non_nil(left.as_ref()) && is_explicit_nil(right.as_ref())
    } else {
        is_present_non_nil(left.as_ref())
            && (is_integer(right.as_ref(), -1) || is_explicit_nil(right.as_ref()))
    }
}

/// `range_from_zero?`: `(irange nil !nil?)` -- a slice from the start that
/// can become a beginless range, e.g. `ary[nil..42]`.
fn range_from_zero(range: &RangeNode<'_>) -> bool {
    !range.is_exclude_end()
        && is_explicit_nil(range.left().as_ref())
        && is_present_non_nil(range.right().as_ref())
}

/// `endless`: `"#{range_node.begin.source}#{range_node.loc.operator.source}"`.
fn endless(range: &RangeNode<'_>, ctx: &Context<'_>) -> String {
    let left = range.left().expect("range_till_minus_one? requires a present left");
    format!("{}{}", text(ctx, left.span()), text(ctx, range.operator_loc().span()))
}

/// `beginless`: `"#{range_node.loc.operator.source}#{range_node.end.source}"`.
fn beginless(range: &RangeNode<'_>, ctx: &Context<'_>) -> String {
    let right = range.right().expect("range_from_zero? requires a present right");
    format!("{}{}", text(ctx, range.operator_loc().span()), text(ctx, right.span()))
}

/// `arguments_source`: since this cop only ever matches a single-argument
/// call, `node.first_argument.source_range.join(node.last_argument...)` is
/// just that one argument's own source.
fn arguments_source(call: &CallNode<'_>, ctx: &Context<'_>) -> String {
    let args = call.arguments().expect("checked by caller");
    let arg = args.arguments().iter().next().expect("checked by caller");
    text(ctx, arg.span())
}

/// `offense_message_for_partial_range`.
fn offense_message_for_partial_range(
    call: &CallNode<'_>,
    prefer: &str,
    offense_range: Span,
    ctx: &Context<'_>,
) -> String {
    let current = if call.call_operator_loc().is_some() {
        arguments_source(call, ctx)
    } else {
        text(ctx, offense_range)
    };
    let prefer =
        if call.call_operator_loc().is_some() { prefer.to_owned() } else { format!("[{prefer}]") };
    MSG_TEMPLATE.replace("{prefer}", &prefer).replace("{current}", &current)
}

/// `offense_message_with_removal_range`.
fn offense_message_with_removal_range(
    target_ruby_version: f32,
    call: &CallNode<'_>,
    range_node: &RangeNode<'_>,
    offense_range: Span,
    ctx: &Context<'_>,
) -> Option<(String, Span)> {
    if range_from_zero_till_minus_one(range_node) {
        let prefer = text(ctx, offense_range);
        Some((MSG_USELESS_TEMPLATE.replace("{prefer}", &prefer), offense_range))
    } else if range_till_minus_one(range_node) {
        let prefer = endless(range_node, ctx);
        let message = offense_message_for_partial_range(call, &prefer, offense_range, ctx);
        let removal_range =
            range_node.right().expect("range_till_minus_one? requires a present right").span();
        Some((message, removal_range))
    } else if range_from_zero(range_node) && target_ruby_version >= 2.7 {
        let prefer = beginless(range_node, ctx);
        let message = offense_message_for_partial_range(call, &prefer, offense_range, ctx);
        let removal_range =
            range_node.left().expect("range_from_zero? requires a present left").span();
        Some((message, removal_range))
    } else {
        None
    }
}

fn text(ctx: &Context<'_>, span: Span) -> String {
    String::from_utf8_lossy(ctx.text(span)).into_owned()
}
