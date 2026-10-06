//! `Performance/RedundantSplitRegexpArgument`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/redundant_split_regexp_argument.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Identifies places where `split` argument can be replaced from a deterministic regexp to a string.
#[derive(Debug, Clone)]
pub struct RedundantSplitRegexpArgument;

impl Rule for RedundantSplitRegexpArgument {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RedundantSplitRegexpArgument",
        department: Department::Performance,
        summary: "Identifies places where `split` argument can be replaced from a deterministic regexp to a string.",
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
