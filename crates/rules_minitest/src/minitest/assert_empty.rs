//! `Minitest/AssertEmpty`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_empty.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `assert_empty` instead of using `assert(object.empty?)`.
#[derive(Debug, Clone)]
pub struct AssertEmpty;

impl Rule for AssertEmpty {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertEmpty",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `assert_empty` instead of using `assert(object.empty?)`.",
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
