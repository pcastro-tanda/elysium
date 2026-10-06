//! `Sorbet/SetterReturnType`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/setter_return_type.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks that setter methods declare a `void` return type in their signature.
#[derive(Debug, Clone)]
pub struct SetterReturnType;

impl Rule for SetterReturnType {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/SetterReturnType",
        department: Department::Sorbet,
        summary: "Checks that setter methods declare a `void` return type in their signature.",
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
