//! `ThreadSafety/RackMiddlewareInstanceVariable`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/rack_middleware_instance_variable.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Avoid instance variables in Rack middleware.
#[derive(Debug, Clone)]
pub struct RackMiddlewareInstanceVariable;

impl Rule for RackMiddlewareInstanceVariable {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/RackMiddlewareInstanceVariable",
        department: Department::ThreadSafety,
        summary: "Avoid instance variables in Rack middleware.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let _ = (node, ctx, NodeKind::CallNode);
    }
}
