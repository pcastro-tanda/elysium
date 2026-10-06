//! `Performance/StringReplacement`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/string_replacement.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `tr` instead of `gsub` when you are replacing the same number of characters. Use `delete` instead of `gsub` when you are deleting characters.
#[derive(Debug, Clone)]
pub struct StringReplacement;

impl Rule for StringReplacement {
    const META: RuleMeta = RuleMeta {
        name: "Performance/StringReplacement",
        department: Department::Performance,
        summary: "Use `tr` instead of `gsub` when you are replacing the same number of characters. Use `delete` instead of `gsub` when you are deleting characters.",
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
