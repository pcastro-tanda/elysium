//! `Performance/SortReverse`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/sort_reverse.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `sort.reverse` instead of `sort { |a, b| b <=> a }`.
#[derive(Debug, Clone)]
pub struct SortReverse;

impl Rule for SortReverse {
    const META: RuleMeta = RuleMeta {
        name: "Performance/SortReverse",
        department: Department::Performance,
        summary: "Use `sort.reverse` instead of `sort { |a, b| b <=> a }`.",
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
