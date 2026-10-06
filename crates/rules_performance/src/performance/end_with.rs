//! `Performance/EndWith`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/end_with.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `end_with?` instead of a regex match anchored to the end of a string.
#[derive(Debug, Clone)]
pub struct EndWith;

impl Rule for EndWith {
    const META: RuleMeta = RuleMeta {
        name: "Performance/EndWith",
        department: Department::Performance,
        summary: "Use `end_with?` instead of a regex match anchored to the end of a string.",
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
