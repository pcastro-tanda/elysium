//! `Rails/WhereNot`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/where_not.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `where.not(...)` instead of manually constructing negated SQL in `where`.
#[derive(Debug, Clone)]
pub struct WhereNot;

impl Rule for WhereNot {
    const META: RuleMeta = RuleMeta {
        name: "Rails/WhereNot",
        department: Department::Rails,
        summary: "Use `where.not(...)` instead of manually constructing negated SQL in `where`.",
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
