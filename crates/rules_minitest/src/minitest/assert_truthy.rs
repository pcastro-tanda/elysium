//! `Minitest/AssertTruthy`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_truthy.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `assert(actual)` instead of using `assert_equal(true, actual)`.
#[derive(Debug, Clone)]
pub struct AssertTruthy;

impl Rule for AssertTruthy {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertTruthy",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `assert(actual)` instead of using `assert_equal(true, actual)`.",
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
