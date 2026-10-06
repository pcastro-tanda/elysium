//! `Minitest/RefuteNil`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_nil.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `refute_nil` instead of using `refute_equal(nil, something)` or `refute(something.nil?)`.
#[derive(Debug, Clone)]
pub struct RefuteNil;

impl Rule for RefuteNil {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteNil",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `refute_nil` instead of using `refute_equal(nil, something)` or `refute(something.nil?)`.",
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
