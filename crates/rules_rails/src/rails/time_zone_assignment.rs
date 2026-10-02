//! `Rails/TimeZoneAssignment`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/time_zone_assignment.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `Time.use_zone` with block instead of `Time.zone=`.";

/// Prefer the usage of `Time.use_zone` instead of manually updating `Time.zone` value.
#[derive(Debug, Clone)]
pub struct TimeZoneAssignment;

impl Rule for TimeZoneAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Rails/TimeZoneAssignment",
        department: Department::Rails,
        summary: "Prefer the usage of `Time.use_zone` instead of manually updating `Time.zone` value.",
        explanation: "Checks for the use of `Time.zone=` method.\n\nThe `zone` attribute persists for the rest of the Ruby runtime, potentially causing unexpected behavior at a later time. Using `Time.use_zone` ensures the code passed in the block is the only place `Time.zone` is affected. It eliminates the possibility of a `zone` sticking around longer than intended.\n\n```ruby\n# bad\nTime.zone = 'EST'\n\n# good\nTime.use_zone('EST') do\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode, NodeKind::CallTargetNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        // `(send (const {nil? cbase} :Time) :zone= ...)`; a multiple-assignment
        // target is such a `send` too.
        let (receiver, name) = if let Some(call) = node.as_call_node() {
            (call.receiver(), call.name())
        } else if let Some(target) = node.as_call_target_node() {
            (Some(target.receiver()), target.name())
        } else {
            return;
        };
        if name.as_slice() != b"zone=" {
            return;
        }
        let Some(receiver) = receiver else { return };
        if is_bare_or_toplevel_const(&receiver) && const_name(&receiver).as_deref() == Some("Time")
        {
            ctx.report(&Self::META, node.span(), MSG);
        }
    }
}
