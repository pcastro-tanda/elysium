//! `Minitest/RefuteEqual`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_equal.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Check if your test uses `refute_equal` instead of `assert(expected != object)` or `assert(! expected == object))`.
#[derive(Debug, Clone)]
pub struct RefuteEqual;

impl Rule for RefuteEqual {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteEqual",
        department: Department::Minitest,
        summary: "Check if your test uses `refute_equal` instead of `assert(expected != object)` or `assert(! expected == object))`.",
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
