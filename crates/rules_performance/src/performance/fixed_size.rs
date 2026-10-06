//! `Performance/FixedSize`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/fixed_size.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Do not compute the size of statically sized objects except in constants.
#[derive(Debug, Clone)]
pub struct FixedSize;

impl Rule for FixedSize {
    const META: RuleMeta = RuleMeta {
        name: "Performance/FixedSize",
        department: Department::Performance,
        summary: "Do not compute the size of statically sized objects except in constants.",
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
