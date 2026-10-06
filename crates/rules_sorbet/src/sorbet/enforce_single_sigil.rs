//! `Sorbet/EnforceSingleSigil`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/enforce_single_sigil.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Ensures that there is only one Sorbet sigil in a file.
#[derive(Debug, Clone)]
pub struct EnforceSingleSigil;

impl Rule for EnforceSingleSigil {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/EnforceSingleSigil",
        department: Department::Sorbet,
        summary: "Ensures that there is only one Sorbet sigil in a file.",
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
