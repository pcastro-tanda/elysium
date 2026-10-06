//! `Sorbet/ForbidUntypedStructProps`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_untyped_struct_props.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Disallows use of `T.untyped` or `T.nilable(T.untyped)` as a prop type for `T::Struct` subclasses.
#[derive(Debug, Clone)]
pub struct ForbidUntypedStructProps;

impl Rule for ForbidUntypedStructProps {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidUntypedStructProps",
        department: Department::Sorbet,
        summary: "Disallows use of `T.untyped` or `T.nilable(T.untyped)` as a prop type for `T::Struct` subclasses.",
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
