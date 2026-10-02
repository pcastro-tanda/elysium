//! `Rails/FindByOrAssignmentMemoization`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/find_by_or_assignment_memoization.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Avoid memoizing `find_by` results with `||=`.
#[derive(Debug, Clone)]
pub struct FindByOrAssignmentMemoization;

impl Rule for FindByOrAssignmentMemoization {
    const META: RuleMeta = RuleMeta {
        name: "Rails/FindByOrAssignmentMemoization",
        department: Department::Rails,
        summary: "Avoid memoizing `find_by` results with `||=`.",
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
