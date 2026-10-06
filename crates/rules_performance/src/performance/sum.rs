//! `Performance/Sum`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/sum.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `sum` instead of a custom array summation.
#[derive(Debug, Clone)]
pub struct Sum;

impl Rule for Sum {
    const META: RuleMeta = RuleMeta {
        name: "Performance/Sum",
        department: Department::Performance,
        summary: "Use `sum` instead of a custom array summation.",
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
