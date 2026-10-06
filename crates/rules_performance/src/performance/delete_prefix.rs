//! `Performance/DeletePrefix`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/delete_prefix.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `delete_prefix` instead of `gsub`.
#[derive(Debug, Clone)]
pub struct DeletePrefix;

impl Rule for DeletePrefix {
    const META: RuleMeta = RuleMeta {
        name: "Performance/DeletePrefix",
        department: Department::Performance,
        summary: "Use `delete_prefix` instead of `gsub`.",
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
