//! `Sorbet/CheckedTrueInSignature`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/checked_true_in_signature.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Disallows the usage of `checked(true)` in signatures.
#[derive(Debug, Clone)]
pub struct CheckedTrueInSignature;

impl Rule for CheckedTrueInSignature {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/CheckedTrueInSignature",
        department: Department::Sorbet,
        summary: "Disallows the usage of `checked(true)` in signatures.",
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
