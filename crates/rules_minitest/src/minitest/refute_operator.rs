//! `Minitest/RefuteOperator`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_operator.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the use of `refute_operator(expected, :<, actual)` over `refute(expected < actual)`.
#[derive(Debug, Clone)]
pub struct RefuteOperator;

impl Rule for RefuteOperator {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteOperator",
        department: Department::Minitest,
        summary: "This cop enforces the use of `refute_operator(expected, :<, actual)` over `refute(expected < actual)`.",
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
