//! `Performance/RangeInclude`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/range_include.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `Range#cover?` instead of `Range#include?` (or `Range#member?`).
#[derive(Debug, Clone)]
pub struct RangeInclude;

impl Rule for RangeInclude {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RangeInclude",
        department: Department::Performance,
        summary: "Use `Range#cover?` instead of `Range#include?` (or `Range#member?`).",
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
