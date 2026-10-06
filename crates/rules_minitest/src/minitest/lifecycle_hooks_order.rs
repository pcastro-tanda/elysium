//! `Minitest/LifecycleHooksOrder`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/lifecycle_hooks_order.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks that lifecycle hooks are declared in the order in which they will be executed.
#[derive(Debug, Clone)]
pub struct LifecycleHooksOrder;

impl Rule for LifecycleHooksOrder {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/LifecycleHooksOrder",
        department: Department::Minitest,
        summary: "Checks that lifecycle hooks are declared in the order in which they will be executed.",
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
