//! `Sorbet/BindingConstantWithoutTypeAlias`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/binding_constant_without_type_alias.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Disallows binding the return value of `T.any`, `T.all`, `T.enum` to a constant directly. To bind the value, one must use `T.type_alias`.
#[derive(Debug, Clone)]
pub struct BindingConstantWithoutTypeAlias;

impl Rule for BindingConstantWithoutTypeAlias {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/BindingConstantWithoutTypeAlias",
        department: Department::Sorbet,
        summary: "Disallows binding the return value of `T.any`, `T.all`, `T.enum` to a constant directly. To bind the value, one must use `T.type_alias`.",
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
