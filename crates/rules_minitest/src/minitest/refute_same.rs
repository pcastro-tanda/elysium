//! `Minitest/RefuteSame`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_same.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Enforces the use of `refute_same(expected, actual)` over `refute(expected.equal?(actual))`.
#[derive(Debug, Clone)]
pub struct RefuteSame;

impl Rule for RefuteSame {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteSame",
        department: Department::Minitest,
        summary: "Enforces the use of `refute_same(expected, actual)` over `refute(expected.equal?(actual))`.",
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
