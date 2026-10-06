//! `Performance/InefficientHashSearch`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/inefficient_hash_search.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `key?` or `value?` instead of `keys.include?` or `values.include?`.
#[derive(Debug, Clone)]
pub struct InefficientHashSearch;

impl Rule for InefficientHashSearch {
    const META: RuleMeta = RuleMeta {
        name: "Performance/InefficientHashSearch",
        department: Department::Performance,
        summary: "Use `key?` or `value?` instead of `keys.include?` or `values.include?`.",
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
