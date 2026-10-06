//! `Performance/StartWith`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/start_with.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `start_with?` instead of a regex match anchored to the beginning of a string.
#[derive(Debug, Clone)]
pub struct StartWith;

impl Rule for StartWith {
    const META: RuleMeta = RuleMeta {
        name: "Performance/StartWith",
        department: Department::Performance,
        summary: "Use `start_with?` instead of a regex match anchored to the beginning of a string.",
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
