//! `Performance/ReverseFirst`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/reverse_first.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `last(n).reverse` instead of `reverse.first(n)`.
#[derive(Debug, Clone)]
pub struct ReverseFirst;

impl Rule for ReverseFirst {
    const META: RuleMeta = RuleMeta {
        name: "Performance/ReverseFirst",
        department: Department::Performance,
        summary: "Use `last(n).reverse` instead of `reverse.first(n)`.",
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
