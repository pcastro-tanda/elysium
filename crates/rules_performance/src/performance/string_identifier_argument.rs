//! `Performance/StringIdentifierArgument`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/string_identifier_argument.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use symbol identifier argument instead of string identifier argument.
#[derive(Debug, Clone)]
pub struct StringIdentifierArgument;

impl Rule for StringIdentifierArgument {
    const META: RuleMeta = RuleMeta {
        name: "Performance/StringIdentifierArgument",
        department: Department::Performance,
        summary: "Use symbol identifier argument instead of string identifier argument.",
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
