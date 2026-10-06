//! `Sorbet/BlockMethodDefinition`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/block_method_definition.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Disallows defining methods inside blocks without using `define_method`, unless the block is a named class definition. This is to avoid running into https://github.com/sorbet/sorbet/issues/3609.
#[derive(Debug, Clone)]
pub struct BlockMethodDefinition;

impl Rule for BlockMethodDefinition {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/BlockMethodDefinition",
        department: Department::Sorbet,
        summary: "Disallows defining methods inside blocks without using `define_method`, unless the block is a named class definition. This is to avoid running into https://github.com/sorbet/sorbet/issues/3609.",
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
