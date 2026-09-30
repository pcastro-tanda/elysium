//! `Layout/SpaceAroundMethodCallOperator`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_around_method_call_operator.rb`.
//!
//! RuboCop's `on_send`/`on_csend` (a `dot?`/`safe_navigation?` guard on
//! `loc.dot`, which is `false` for a `::`-scoped method call) collapses to a
//! single `CallNode` arm here, since Prism folds every send spelling into one
//! node shape distinguished by `call_operator_loc`'s text. RuboCop's
//! `on_const` (checking `loc?(:double_colon)`, true for every `const` node
//! scoped by an explicit or implicit leading `::`) maps onto Prism's
//! `ConstantPathNode`, whose `delimiter_loc` (the node's own leading `::`) is
//! mandatory -- so every qualified constant segment, including a bare
//! leading `::foo`, is visited uniformly without a `parent().is_none()`
//! special case.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Avoid using spaces around a method call operator.";

/// Checks method call operators to not have spaces around them.
#[derive(Debug, Clone)]
pub struct SpaceAroundMethodCallOperator;

impl SpaceAroundMethodCallOperator {
    /// RuboCop's `check_space`: `range_between(begin_pos, end_pos)`, offense
    /// only when that whole range is one or more spaces/tabs (RuboCop's
    /// `SPACES_REGEXP = /\A[ \t]+\z/`, which never matches a range spanning a
    /// newline, comment, or any other non-blank text).
    fn check_space(ctx: &mut Context<'_>, begin_pos: u32, end_pos: u32) {
        if end_pos <= begin_pos {
            return;
        }
        let span = Span::new(begin_pos, end_pos);
        if !ctx.text(span).iter().all(|&b| b == b' ' || b == b'\t') {
            return;
        }
        let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }

    /// RuboCop's `check_space_before_dot`.
    fn check_space_before_dot(ctx: &mut Context<'_>, call: &CallNode<'_>, dot_pos: u32) {
        let Some(receiver) = call.receiver() else { return };
        Self::check_space(ctx, receiver.span().end, dot_pos);
    }

    /// RuboCop's `check_space_after_dot`: the "Proc#call" shorthand
    /// (`foo.(1, 2)`) has no `message_loc`, so the selector position falls
    /// back to the call's own opening delimiter (`(`).
    fn check_space_after_dot(ctx: &mut Context<'_>, call: &CallNode<'_>, dot_end: u32) {
        let selector_pos = if call.name().as_slice() == b"call" && call.message_loc().is_none() {
            call.opening_loc().map(|loc| loc.span().start)
        } else {
            call.message_loc().map(|loc| loc.span().start)
        };
        if let Some(selector_pos) = selector_pos {
            Self::check_space(ctx, dot_end, selector_pos);
        }
    }
}

impl Rule for SpaceAroundMethodCallOperator {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceAroundMethodCallOperator",
        department: Department::Layout,
        summary: "Checks method call operators to not have spaces around them.",
        explanation: "\
```ruby
# bad
foo. bar
foo .bar
foo . bar
foo. bar .buzz
foo
  . bar
  . buzz
foo&. bar
foo &.bar
foo &. bar
foo &. bar&. buzz
RuboCop:: Cop
RuboCop:: Cop:: Base
:: RuboCop::Cop

# good
foo.bar
foo.bar.buzz
foo
  .bar
  .buzz
foo&.bar
foo&.bar&.buzz
RuboCop::Cop
RuboCop::Cop::Base
::RuboCop::Cop
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::ConstantPathNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("CallNode kind");
                let Some(op_loc) = call.call_operator_loc() else { return };
                if !matches!(op_loc.as_slice(), b"." | b"&.") {
                    return;
                }
                let op_span = op_loc.span();
                Self::check_space_before_dot(ctx, &call, op_span.start);
                Self::check_space_after_dot(ctx, &call, op_span.end);
            }
            NodeKind::ConstantPathNode => {
                let path = node.as_constant_path_node().expect("ConstantPathNode kind");
                let double_colon_end = path.delimiter_loc().span().end;
                let name_start = path.name_loc().span().start;
                Self::check_space(ctx, double_colon_end, name_start);
            }
            _ => {}
        }
    }
}
