//! `Performance/ReverseEach`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/reverse_each.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `reverse_each` instead of `reverse.each`.
#[derive(Debug, Clone)]
pub struct ReverseEach;

impl Rule for ReverseEach {
    const META: RuleMeta = RuleMeta {
        name: "Performance/ReverseEach",
        department: Department::Performance,
        summary: "Use `reverse_each` instead of `reverse.each`.",
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
