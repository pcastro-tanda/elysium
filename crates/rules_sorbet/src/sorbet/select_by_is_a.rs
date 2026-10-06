//! `Sorbet/SelectByIsA`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/select_by_is_a.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Suggests using `grep` over `select` when using it only for type narrowing. This is because Sorbet can properly infer types when using `grep` but not with `select`.
#[derive(Debug, Clone)]
pub struct SelectByIsA;

impl Rule for SelectByIsA {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/SelectByIsA",
        department: Department::Sorbet,
        summary: "Suggests using `grep` over `select` when using it only for type narrowing. This is because Sorbet can properly infer types when using `grep` but not with `select`.",
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
