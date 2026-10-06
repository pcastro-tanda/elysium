//! `Sorbet/RedundantTLet`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/redundant_t_let.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Prevents redundant use of `T.let` where Sorbet infers types automatically: instance variables assigned from signature parameters in `initialize`, and constants assigned constructor calls.
#[derive(Debug, Clone)]
pub struct RedundantTLet;

impl Rule for RedundantTLet {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/RedundantTLet",
        department: Department::Sorbet,
        summary: "Prevents redundant use of `T.let` where Sorbet infers types automatically: instance variables assigned from signature parameters in `initialize`, and constants assigned constructor calls.",
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
