//! `Rails/BulkChangeTable`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/bulk_change_table.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Check whether alter queries are combinable.
#[derive(Debug, Clone)]
pub struct BulkChangeTable;

impl Rule for BulkChangeTable {
    const META: RuleMeta = RuleMeta {
        name: "Rails/BulkChangeTable",
        department: Department::Rails,
        summary: "Check whether alter queries are combinable.",
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
