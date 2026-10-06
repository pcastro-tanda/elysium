//! `Sorbet/SignatureBuildOrder`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/signature_build_order.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Enforces the order of parts in a signature.
The order is first inheritance related builders, then params, then return and finally the modifier such as: `abstract.params(...).returns(...).soft`.'
#[derive(Debug, Clone)]
pub struct SignatureBuildOrder;

impl Rule for SignatureBuildOrder {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/SignatureBuildOrder",
        department: Department::Sorbet,
        summary: "Enforces the order of parts in a signature.
The order is first inheritance related builders, then params, then return and finally the modifier such as: `abstract.params(...).returns(...).soft`.'",
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
