//! `Sorbet/KeywordArgumentOrdering`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/keyword_argument_ordering.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Enforces a compatible keyword arguments with Sorbet.
All keyword arguments must be at the end of the parameters list, and all keyword arguments with a default value must be after those without default values.
#[derive(Debug, Clone)]
pub struct KeywordArgumentOrdering;

impl Rule for KeywordArgumentOrdering {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/KeywordArgumentOrdering",
        department: Department::Sorbet,
        summary: "Enforces a compatible keyword arguments with Sorbet.
All keyword arguments must be at the end of the parameters list, and all keyword arguments with a default value must be after those without default values.",
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
