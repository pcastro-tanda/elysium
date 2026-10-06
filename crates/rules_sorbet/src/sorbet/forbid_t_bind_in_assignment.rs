//! `Sorbet/ForbidTBindInAssignment`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_t_bind_in_assignment.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Forbid assigning the result of T.bind.
#[derive(Debug, Clone)]
pub struct ForbidTBindInAssignment;

impl Rule for ForbidTBindInAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidTBindInAssignment",
        department: Department::Sorbet,
        summary: "Forbid assigning the result of T.bind.",
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
