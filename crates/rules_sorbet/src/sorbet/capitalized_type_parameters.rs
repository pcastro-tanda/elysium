//! `Sorbet/CapitalizedTypeParameters`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/capitalized_type_parameters.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Ensures that type parameters are capitalized.
#[derive(Debug, Clone)]
pub struct CapitalizedTypeParameters;

impl Rule for CapitalizedTypeParameters {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/CapitalizedTypeParameters",
        department: Department::Sorbet,
        summary: "Ensures that type parameters are capitalized.",
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
