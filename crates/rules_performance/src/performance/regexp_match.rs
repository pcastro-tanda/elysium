//! `Performance/RegexpMatch`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/regexp_match.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `match?` instead of `Regexp#match`, `String#match`, `Symbol#match`, `Regexp#===`, or `=~` when `MatchData` is not used.
#[derive(Debug, Clone)]
pub struct RegexpMatch;

impl Rule for RegexpMatch {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RegexpMatch",
        department: Department::Performance,
        summary: "Use `match?` instead of `Regexp#match`, `String#match`, `Symbol#match`, `Regexp#===`, or `=~` when `MatchData` is not used.",
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
