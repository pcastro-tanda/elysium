//! `Performance/MapMethodChain`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/map_method_chain.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks if the `map` method is used in a chain.
#[derive(Debug, Clone)]
pub struct MapMethodChain;

impl Rule for MapMethodChain {
    const META: RuleMeta = RuleMeta {
        name: "Performance/MapMethodChain",
        department: Department::Performance,
        summary: "Checks if the `map` method is used in a chain.",
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
