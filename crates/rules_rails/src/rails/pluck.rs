//! `Rails/Pluck`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/pluck.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Prefer `pluck` over `map { ... }`.
#[derive(Debug, Clone)]
pub struct Pluck;

impl Rule for Pluck {
    const META: RuleMeta = RuleMeta {
        name: "Rails/Pluck",
        department: Department::Rails,
        summary: "Prefer `pluck` over `map { ... }`.",
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
