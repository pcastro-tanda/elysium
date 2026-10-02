//! `Rails/WhereRange`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/where_range.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use ranges in `where` instead of manually constructing SQL.
#[derive(Debug, Clone)]
pub struct WhereRange;

impl Rule for WhereRange {
    const META: RuleMeta = RuleMeta {
        name: "Rails/WhereRange",
        department: Department::Rails,
        summary: "Use ranges in `where` instead of manually constructing SQL.",
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
