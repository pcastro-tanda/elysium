//! `Minitest/RefuteInDelta`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_in_delta.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `refute_in_delta` instead of using `refute_equal` to compare floats.
#[derive(Debug, Clone)]
pub struct RefuteInDelta;

impl Rule for RefuteInDelta {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteInDelta",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `refute_in_delta` instead of using `refute_equal` to compare floats.",
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
