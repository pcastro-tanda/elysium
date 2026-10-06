//! `Rails/WhereExists`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/where_exists.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Prefer `exists?(...)` over `where(...).exists?`.
#[derive(Debug, Clone)]
pub struct WhereExists;

impl Rule for WhereExists {
    const META: RuleMeta = RuleMeta {
        name: "Rails/WhereExists",
        department: Department::Rails,
        summary: "Prefer `exists?(...)` over `where(...).exists?`.",
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
