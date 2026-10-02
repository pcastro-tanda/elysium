//! `Rails/NotNullColumn`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/not_null_column.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Do not add a NOT NULL column without a default value to existing tables.
#[derive(Debug, Clone)]
pub struct NotNullColumn;

impl Rule for NotNullColumn {
    const META: RuleMeta = RuleMeta {
        name: "Rails/NotNullColumn",
        department: Department::Rails,
        summary: "Do not add a NOT NULL column without a default value to existing tables.",
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
