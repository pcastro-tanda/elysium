//! `Sorbet/ObsoleteStrictMemoization`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/obsolete_strict_memoization.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop checks for the obsolete pattern for initializing instance variables that was required for older Sorbet versions in `#typed: strict` files.
It's no longer required, as of Sorbet 0.5.10210 See https://sorbet.org/docs/type-assertions#put-type-assertions-behind-memoization
#[derive(Debug, Clone)]
pub struct ObsoleteStrictMemoization;

impl Rule for ObsoleteStrictMemoization {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ObsoleteStrictMemoization",
        department: Department::Sorbet,
        summary: "This cop checks for the obsolete pattern for initializing instance variables that was required for older Sorbet versions in `#typed: strict` files.
It's no longer required, as of Sorbet 0.5.10210 See https://sorbet.org/docs/type-assertions#put-type-assertions-behind-memoization",
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
