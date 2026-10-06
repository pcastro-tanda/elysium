//! `Minitest/ReturnInTestMethod`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/return_in_test_method.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Enforces the use of `skip` instead of `return` in test methods.
#[derive(Debug, Clone)]
pub struct ReturnInTestMethod;

impl Rule for ReturnInTestMethod {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/ReturnInTestMethod",
        department: Department::Minitest,
        summary: "Enforces the use of `skip` instead of `return` in test methods.",
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
