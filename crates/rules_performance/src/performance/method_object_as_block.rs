//! `Performance/MethodObjectAsBlock`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/method_object_as_block.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use block explicitly instead of block-passing a method object.
#[derive(Debug, Clone)]
pub struct MethodObjectAsBlock;

impl Rule for MethodObjectAsBlock {
    const META: RuleMeta = RuleMeta {
        name: "Performance/MethodObjectAsBlock",
        department: Department::Performance,
        summary: "Use block explicitly instead of block-passing a method object.",
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
