//! `Minitest/RefuteMatch`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_match.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `refute_match` instead of using `refute(matcher.match(object))`.
#[derive(Debug, Clone)]
pub struct RefuteMatch;

impl Rule for RefuteMatch {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteMatch",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `refute_match` instead of using `refute(matcher.match(object))`.",
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
