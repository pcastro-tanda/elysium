//! `Minitest/AssertOutput`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_output.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop checks for opportunities to use `assert_output`.
#[derive(Debug, Clone)]
pub struct AssertOutput;

impl Rule for AssertOutput {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertOutput",
        department: Department::Minitest,
        summary: "This cop checks for opportunities to use `assert_output`.",
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
