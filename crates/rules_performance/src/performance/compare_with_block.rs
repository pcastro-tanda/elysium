//! `Performance/CompareWithBlock`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/compare_with_block.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `sort_by(&:foo)` instead of `sort { |a, b| a.foo <=> b.foo }`.
#[derive(Debug, Clone)]
pub struct CompareWithBlock;

impl Rule for CompareWithBlock {
    const META: RuleMeta = RuleMeta {
        name: "Performance/CompareWithBlock",
        department: Department::Performance,
        summary: "Use `sort_by(&:foo)` instead of `sort { |a, b| a.foo <=> b.foo }`.",
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
