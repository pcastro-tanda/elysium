//! `Sorbet/StrictSigil`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/strict_sigil.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// All files must be at least at strictness `strict`.
#[derive(Debug, Clone)]
pub struct StrictSigil;

impl Rule for StrictSigil {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/StrictSigil",
        department: Department::Sorbet,
        summary: "All files must be at least at strictness `strict`.",
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
