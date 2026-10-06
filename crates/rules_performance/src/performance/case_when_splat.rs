//! `Performance/CaseWhenSplat`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/case_when_splat.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Reordering `when` conditions with a splat to the end of the `when` branches can improve performance.
#[derive(Debug, Clone)]
pub struct CaseWhenSplat;

impl Rule for CaseWhenSplat {
    const META: RuleMeta = RuleMeta {
        name: "Performance/CaseWhenSplat",
        department: Department::Performance,
        summary: "Reordering `when` conditions with a splat to the end of the `when` branches can improve performance.",
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
