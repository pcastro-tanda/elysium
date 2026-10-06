//! `Minitest/UselessAssertion`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/useless_assertion.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Detects useless assertions (assertions that either always pass or always fail).
#[derive(Debug, Clone)]
pub struct UselessAssertion;

impl Rule for UselessAssertion {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/UselessAssertion",
        department: Department::Minitest,
        summary: "Detects useless assertions (assertions that either always pass or always fail).",
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
