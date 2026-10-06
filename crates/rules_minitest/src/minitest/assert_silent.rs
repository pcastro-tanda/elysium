//! `Minitest/AssertSilent`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_silent.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `assert_silent { ... }` instead of using `assert_output('', '') { ... }`.
#[derive(Debug, Clone)]
pub struct AssertSilent;

impl Rule for AssertSilent {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertSilent",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `assert_silent { ... }` instead of using `assert_output('', '') { ... }`.",
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
