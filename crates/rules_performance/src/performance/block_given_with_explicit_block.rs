//! `Performance/BlockGivenWithExplicitBlock`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/block_given_with_explicit_block.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Check block argument explicitly instead of using `block_given?`.
#[derive(Debug, Clone)]
pub struct BlockGivenWithExplicitBlock;

impl Rule for BlockGivenWithExplicitBlock {
    const META: RuleMeta = RuleMeta {
        name: "Performance/BlockGivenWithExplicitBlock",
        department: Department::Performance,
        summary: "Check block argument explicitly instead of using `block_given?`.",
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
