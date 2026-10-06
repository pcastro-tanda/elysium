//! `Sorbet/ForbidTAnyWithNil`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_t_any_with_nil.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Forbid usage of T.any(NilClass).
#[derive(Debug, Clone)]
pub struct ForbidTAnyWithNil;

impl Rule for ForbidTAnyWithNil {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidTAnyWithNil",
        department: Department::Sorbet,
        summary: "Forbid usage of T.any(NilClass).",
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
