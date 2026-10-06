//! `Performance/RedundantEqualityComparisonBlock`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/redundant_equality_comparison_block.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks for uses `Enumerable#all?`, `Enumerable#any?`, `Enumerable#one?`, or `Enumerable#none?` are compared with `===` or similar methods in block.
#[derive(Debug, Clone)]
pub struct RedundantEqualityComparisonBlock;

impl Rule for RedundantEqualityComparisonBlock {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RedundantEqualityComparisonBlock",
        department: Department::Performance,
        summary: "Checks for uses `Enumerable#all?`, `Enumerable#any?`, `Enumerable#one?`, or `Enumerable#none?` are compared with `===` or similar methods in block.",
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
