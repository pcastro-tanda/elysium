//! `Minitest/EmptyLineBeforeAssertionMethods`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/empty_line_before_assertion_methods.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Add empty line before assertion methods.
#[derive(Debug, Clone)]
pub struct EmptyLineBeforeAssertionMethods;

impl Rule for EmptyLineBeforeAssertionMethods {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/EmptyLineBeforeAssertionMethods",
        department: Department::Minitest,
        summary: "Add empty line before assertion methods.",
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
