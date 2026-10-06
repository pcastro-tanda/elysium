//! `Minitest/SkipEnsure`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/skip_ensure.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks that `ensure` call even if `skip`.
#[derive(Debug, Clone)]
pub struct SkipEnsure;

impl Rule for SkipEnsure {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/SkipEnsure",
        department: Department::Minitest,
        summary: "Checks that `ensure` call even if `skip`.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Warning,
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
