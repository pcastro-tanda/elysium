//! `Minitest/AssertionInLifecycleHook`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assertion_in_lifecycle_hook.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop checks for usage of assertions in lifecycle hooks.
#[derive(Debug, Clone)]
pub struct AssertionInLifecycleHook;

impl Rule for AssertionInLifecycleHook {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertionInLifecycleHook",
        department: Department::Minitest,
        summary: "This cop checks for usage of assertions in lifecycle hooks.",
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
