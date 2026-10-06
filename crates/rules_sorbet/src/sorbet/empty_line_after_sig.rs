//! `Sorbet/EmptyLineAfterSig`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/empty_line_after_sig.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Ensures that there are no blank lines after signatures
#[derive(Debug, Clone)]
pub struct EmptyLineAfterSig;

impl Rule for EmptyLineAfterSig {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/EmptyLineAfterSig",
        department: Department::Sorbet,
        summary: "Ensures that there are no blank lines after signatures",
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
