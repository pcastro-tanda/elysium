//! `Rails/CompactBlank`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/compact_blank.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks if collection can be blank-compacted with `compact_blank`.
#[derive(Debug, Clone)]
pub struct CompactBlank;

impl Rule for CompactBlank {
    const META: RuleMeta = RuleMeta {
        name: "Rails/CompactBlank",
        department: Department::Rails,
        summary: "Checks if collection can be blank-compacted with `compact_blank`.",
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
