//! `Layout/SpaceAfterNot`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_after_not.rb`.
//!
//! Upstream restricts `on_send` to `RESTRICT_ON_SEND = %i[!]` and further
//! filters with `prefix_bang?` (`negation_method?` -- has a receiver and the
//! method name is `:!` -- plus `loc.selector.is?('!')`, excluding the
//! keyword spelling `not`). A dot-call spelling like `x.!` also satisfies
//! that predicate, but is excluded for free by `whitespace_after_operator?`
//! itself: `x.!`'s own source range starts at `x` (the receiver), the same
//! place its receiver starts, so the gap is always zero there. Prism folds
//! the bare `!x` and dotted `x.!` spellings into the same
//! [`NodeKind::CallNode`] shape distinguished only by `call_operator_loc`,
//! so the same gap check (comparing the whole call's span start against its
//! receiver's span start) reproduces the same exclusion here.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Do not leave space between `!` and its argument.";

/// Tracks redundant space after the ! operator.
#[derive(Debug, Clone)]
pub struct SpaceAfterNot;

impl Rule for SpaceAfterNot {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceAfterNot",
        department: Department::Layout,
        summary: "Tracks redundant space after the ! operator.",
        explanation: "Checks for space after `!`.\n\n\
```ruby\n# bad\n! something\n\n# good\n!something\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        let Some(message) = call.message_loc() else { return };
        let message_span = message.span();
        if ctx.text(message_span) != b"!" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let node_span = node.span();
        let receiver_span = receiver.span();
        if receiver_span.start.saturating_sub(node_span.start) <= 1 {
            return;
        }
        let gap = Span::new(message_span.end, receiver_span.start);
        ctx.report_with_fix(
            &Self::META,
            node_span,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(gap)] },
        );
    }
}
