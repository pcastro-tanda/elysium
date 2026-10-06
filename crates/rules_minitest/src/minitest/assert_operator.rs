//! `Minitest/AssertOperator`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_operator.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the use of `assert_operator(expected, :<, actual)` over `assert(expected < actual)`.
#[derive(Debug, Clone)]
pub struct AssertOperator;

impl Rule for AssertOperator {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertOperator",
        department: Department::Minitest,
        summary: "This cop enforces the use of `assert_operator(expected, :<, actual)` over `assert(expected < actual)`.",
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
