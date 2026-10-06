//! `Performance/RedundantMerge`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/redundant_merge.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use Hash#[]=, rather than Hash#merge! with a single key-value pair.
#[derive(Debug, Clone)]
pub struct RedundantMerge;

impl Rule for RedundantMerge {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RedundantMerge",
        department: Department::Performance,
        summary: "Use Hash#[]=, rather than Hash#merge! with a single key-value pair.",
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
