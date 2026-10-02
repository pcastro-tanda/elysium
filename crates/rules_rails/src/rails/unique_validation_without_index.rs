//! `Rails/UniqueValidationWithoutIndex`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/unique_validation_without_index.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Uniqueness validation should have a unique index on the database column.
#[derive(Debug, Clone)]
pub struct UniqueValidationWithoutIndex;

impl Rule for UniqueValidationWithoutIndex {
    const META: RuleMeta = RuleMeta {
        name: "Rails/UniqueValidationWithoutIndex",
        department: Department::Rails,
        summary: "Uniqueness validation should have a unique index on the database column.",
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
