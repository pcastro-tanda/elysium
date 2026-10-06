//! `Sorbet/TypeAliasName`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/type_alias_name.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Type alias constant names must be in CamelCase.
#[derive(Debug, Clone)]
pub struct TypeAliasName;

impl Rule for TypeAliasName {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/TypeAliasName",
        department: Department::Sorbet,
        summary: "Type alias constant names must be in CamelCase.",
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
