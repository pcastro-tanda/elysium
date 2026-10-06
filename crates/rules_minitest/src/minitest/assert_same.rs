//! `Minitest/AssertSame`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_same.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Enforces the use of `assert_same(expected, actual)` over `assert(expected.equal?(actual))`.
#[derive(Debug, Clone)]
pub struct AssertSame;

impl Rule for AssertSame {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertSame",
        department: Department::Minitest,
        summary: "Enforces the use of `assert_same(expected, actual)` over `assert(expected.equal?(actual))`.",
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
