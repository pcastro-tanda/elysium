//! `Minitest/LiteralAsActualArgument`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/literal_as_actual_argument.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces correct order of `expected` and `actual` arguments for `assert_equal`.
#[derive(Debug, Clone)]
pub struct LiteralAsActualArgument;

impl Rule for LiteralAsActualArgument {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/LiteralAsActualArgument",
        department: Department::Minitest,
        summary: "This cop enforces correct order of `expected` and `actual` arguments for `assert_equal`.",
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
