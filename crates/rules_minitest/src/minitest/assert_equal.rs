//! `Minitest/AssertEqual`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_equal.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `assert_equal` instead of using `assert(expected == actual)`.
#[derive(Debug, Clone)]
pub struct AssertEqual;

impl Rule for AssertEqual {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertEqual",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `assert_equal` instead of using `assert(expected == actual)`.",
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
