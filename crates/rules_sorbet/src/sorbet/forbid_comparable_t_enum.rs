//! `Sorbet/ForbidComparableTEnum`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_comparable_t_enum.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Disallows including the `Comparable` module in a `T::Enum`.
#[derive(Debug, Clone)]
pub struct ForbidComparableTEnum;

impl Rule for ForbidComparableTEnum {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidComparableTEnum",
        department: Department::Sorbet,
        summary: "Disallows including the `Comparable` module in a `T::Enum`.",
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
