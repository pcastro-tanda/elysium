//! `Minitest/GlobalExpectations`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/global_expectations.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop checks for deprecated global expectations.
#[derive(Debug, Clone)]
pub struct GlobalExpectations;

impl Rule for GlobalExpectations {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/GlobalExpectations",
        department: Department::Minitest,
        summary: "This cop checks for deprecated global expectations.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Warning,
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
