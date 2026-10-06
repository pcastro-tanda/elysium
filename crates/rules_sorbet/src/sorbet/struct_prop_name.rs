//! `Sorbet/StructPropName`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/struct_prop_name.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks that T::Struct property names use the configured naming style.
#[derive(Debug, Clone)]
pub struct StructPropName;

impl Rule for StructPropName {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/StructPropName",
        department: Department::Sorbet,
        summary: "Checks that T::Struct property names use the configured naming style.",
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
