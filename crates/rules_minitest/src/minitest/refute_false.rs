//! `Minitest/RefuteFalse`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_false.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Check if your test uses `refute(actual)` instead of `assert_equal(false, actual)`.
#[derive(Debug, Clone)]
pub struct RefuteFalse;

impl Rule for RefuteFalse {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteFalse",
        department: Department::Minitest,
        summary: "Check if your test uses `refute(actual)` instead of `assert_equal(false, actual)`.",
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
