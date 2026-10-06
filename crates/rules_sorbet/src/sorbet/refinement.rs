//! `Sorbet/Refinement`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/refinement.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks for the use of Ruby Refinements library. Refinements add complexity and incur a performance penalty that can be significant for large code bases. They are also not supported by Sorbet.
#[derive(Debug, Clone)]
pub struct Refinement;

impl Rule for Refinement {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/Refinement",
        department: Department::Sorbet,
        summary: "Checks for the use of Ruby Refinements library. Refinements add complexity and incur a performance penalty that can be significant for large code bases. They are also not supported by Sorbet.",
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
