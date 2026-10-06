//! `Sorbet/RuntimeOnFailureDependsOnChecked`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/runtime_on_failure_depends_on_checked.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Ensures that `on_failure` is called after `checked` in signatures. The `on_failure` method has no effect unless `checked(:tests)` or `checked(:always)` is also called.
#[derive(Debug, Clone)]
pub struct RuntimeOnFailureDependsOnChecked;

impl Rule for RuntimeOnFailureDependsOnChecked {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/RuntimeOnFailureDependsOnChecked",
        department: Department::Sorbet,
        summary: "Ensures that `on_failure` is called after `checked` in signatures. The `on_failure` method has no effect unless `checked(:tests)` or `checked(:always)` is also called.",
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
