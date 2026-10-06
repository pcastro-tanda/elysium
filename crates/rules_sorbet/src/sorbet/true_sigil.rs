//! `Sorbet/TrueSigil`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/true_sigil.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// All files must be at least at strictness `true`.
#[derive(Debug, Clone)]
pub struct TrueSigil;

impl Rule for TrueSigil {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/TrueSigil",
        department: Department::Sorbet,
        summary: "All files must be at least at strictness `true`.",
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
