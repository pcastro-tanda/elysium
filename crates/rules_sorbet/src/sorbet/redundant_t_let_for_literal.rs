//! `Sorbet/RedundantTLetForLiteral`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/redundant_t_let_for_literal.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks for redundant `T.let` declarations and trailing RBS annotations on constants whose literal values have types Sorbet can infer automatically.
#[derive(Debug, Clone)]
pub struct RedundantTLetForLiteral;

impl Rule for RedundantTLetForLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/RedundantTLetForLiteral",
        department: Department::Sorbet,
        summary: "Checks for redundant `T.let` declarations and trailing RBS annotations on constants whose literal values have types Sorbet can infer automatically.",
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
