//! `Performance/StringInclude`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/string_include.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `String#include?` instead of a regex match with literal-only pattern.
#[derive(Debug, Clone)]
pub struct StringInclude;

impl Rule for StringInclude {
    const META: RuleMeta = RuleMeta {
        name: "Performance/StringInclude",
        department: Department::Performance,
        summary: "Use `String#include?` instead of a regex match with literal-only pattern.",
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
