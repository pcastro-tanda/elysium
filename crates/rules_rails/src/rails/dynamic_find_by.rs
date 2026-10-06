//! `Rails/DynamicFindBy`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/dynamic_find_by.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `find_by` instead of dynamic `find_by_*`.
#[derive(Debug, Clone)]
pub struct DynamicFindBy;

impl Rule for DynamicFindBy {
    const META: RuleMeta = RuleMeta {
        name: "Rails/DynamicFindBy",
        department: Department::Rails,
        summary: "Use `find_by` instead of dynamic `find_by_*`.",
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
