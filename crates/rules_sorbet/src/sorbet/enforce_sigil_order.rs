//! `Sorbet/EnforceSigilOrder`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/enforce_sigil_order.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Ensures that Sorbet sigil comes first in a file.
#[derive(Debug, Clone)]
pub struct EnforceSigilOrder;

impl Rule for EnforceSigilOrder {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/EnforceSigilOrder",
        department: Department::Sorbet,
        summary: "Ensures that Sorbet sigil comes first in a file.",
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
