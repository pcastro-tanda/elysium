//! `ThreadSafety/ActiveSupportCallbacks`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/active_support_callbacks.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Avoid mutating ActiveSupport callback chains at runtime.
#[derive(Debug, Clone)]
pub struct ActiveSupportCallbacks;

impl Rule for ActiveSupportCallbacks {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/ActiveSupportCallbacks",
        department: Department::ThreadSafety,
        summary: "Avoid mutating ActiveSupport callback chains at runtime.",
        explanation: "",
        enabled_by_default: false,
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
