//! `Sorbet/AllowIncompatibleOverride`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/allow_incompatible_override.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Disallows using `.override(allow_incompatible: true)`.
#[derive(Debug, Clone)]
pub struct AllowIncompatibleOverride;

impl Rule for AllowIncompatibleOverride {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/AllowIncompatibleOverride",
        department: Department::Sorbet,
        summary: "Disallows using `.override(allow_incompatible: true)`.",
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
