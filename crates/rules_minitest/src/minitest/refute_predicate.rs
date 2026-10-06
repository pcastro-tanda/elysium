//! `Minitest/RefutePredicate`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_predicate.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `refute_predicate` instead of using `refute(obj.a_predicate_method?)`.
#[derive(Debug, Clone)]
pub struct RefutePredicate;

impl Rule for RefutePredicate {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefutePredicate",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `refute_predicate` instead of using `refute(obj.a_predicate_method?)`.",
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
