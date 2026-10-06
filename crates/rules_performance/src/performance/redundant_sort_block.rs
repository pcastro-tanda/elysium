//! `Performance/RedundantSortBlock`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/redundant_sort_block.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `sort` instead of `sort { |a, b| a <=> b }`.
#[derive(Debug, Clone)]
pub struct RedundantSortBlock;

impl Rule for RedundantSortBlock {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RedundantSortBlock",
        department: Department::Performance,
        summary: "Use `sort` instead of `sort { |a, b| a <=> b }`.",
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
