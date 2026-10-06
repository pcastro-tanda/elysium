//! `Performance/DoubleStartEndWith`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/double_start_end_with.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `str.{start,end}_with?(x, ..., y, ...)` instead of `str.{start,end}_with?(x, ...) || str.{start,end}_with?(y, ...)`.
#[derive(Debug, Clone)]
pub struct DoubleStartEndWith;

impl Rule for DoubleStartEndWith {
    const META: RuleMeta = RuleMeta {
        name: "Performance/DoubleStartEndWith",
        department: Department::Performance,
        summary: "Use `str.{start,end}_with?(x, ..., y, ...)` instead of `str.{start,end}_with?(x, ...) || str.{start,end}_with?(y, ...)`.",
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
