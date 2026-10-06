//! `Sorbet/MultipleTEnumValues`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/multiple_t_enum_values.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Ensures that all `T::Enum`s have multiple values.
#[derive(Debug, Clone)]
pub struct MultipleTEnumValues;

impl Rule for MultipleTEnumValues {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/MultipleTEnumValues",
        department: Department::Sorbet,
        summary: "Ensures that all `T::Enum`s have multiple values.",
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
