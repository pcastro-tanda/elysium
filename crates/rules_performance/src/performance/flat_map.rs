//! `Performance/FlatMap`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/flat_map.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `Enumerable#flat_map` instead of `Enumerable#map...Array#flatten(1)` or `Enumerable#collect..Array#flatten(1)`.
#[derive(Debug, Clone)]
pub struct FlatMap;

impl Rule for FlatMap {
    const META: RuleMeta = RuleMeta {
        name: "Performance/FlatMap",
        department: Department::Performance,
        summary: "Use `Enumerable#flat_map` instead of `Enumerable#map...Array#flatten(1)` or `Enumerable#collect..Array#flatten(1)`.",
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
