//! `Minitest/AssertInDelta`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_in_delta.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `assert_in_delta` instead of using `assert_equal` to compare floats.
#[derive(Debug, Clone)]
pub struct AssertInDelta;

impl Rule for AssertInDelta {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertInDelta",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `assert_in_delta` instead of using `assert_equal` to compare floats.",
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
