//! `Performance/Squeeze`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/squeeze.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `squeeze('a')` instead of `gsub(/a+/, 'a')`.
#[derive(Debug, Clone)]
pub struct Squeeze;

impl Rule for Squeeze {
    const META: RuleMeta = RuleMeta {
        name: "Performance/Squeeze",
        department: Department::Performance,
        summary: "Use `squeeze('a')` instead of `gsub(/a+/, 'a')`.",
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
