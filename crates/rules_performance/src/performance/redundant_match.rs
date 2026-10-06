//! `Performance/RedundantMatch`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/redundant_match.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `=~` instead of `String#match` or `Regexp#match` in a context where the returned `MatchData` is not needed.
#[derive(Debug, Clone)]
pub struct RedundantMatch;

impl Rule for RedundantMatch {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RedundantMatch",
        department: Department::Performance,
        summary: "Use `=~` instead of `String#match` or `Regexp#match` in a context where the returned `MatchData` is not needed.",
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
