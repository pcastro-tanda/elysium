//! `Sorbet/BuggyObsoleteStrictMemoization`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/buggy_obsolete_strict_memoization.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks for the a mistaken variant of the "obsolete memoization pattern" that used to be required for older Sorbet versions in `#typed: strict` files. The mistaken variant would overwrite the ivar with `nil` on every call, causing the memoized value to be discarded and recomputed on every call.
/// This cop will correct it to read from the ivar instead of `nil`, which will memoize it correctly.
/// The result of this correction will be the "obsolete memoization pattern", which can further be corrected by the `Sorbet/ObsoleteStrictMemoization` cop.
/// See `Sorbet/ObsoleteStrictMemoization` for more details.
#[derive(Debug, Clone)]
pub struct BuggyObsoleteStrictMemoization;

impl Rule for BuggyObsoleteStrictMemoization {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/BuggyObsoleteStrictMemoization",
        department: Department::Sorbet,
        summary: "Checks for the a mistaken variant of the \"obsolete memoization pattern\" that used to be required for older Sorbet versions in `#typed: strict` files. The mistaken variant would overwrite the ivar with `nil` on every call, causing the memoized value to be discarded and recomputed on every call.
This cop will correct it to read from the ivar instead of `nil`, which will memoize it correctly.
The result of this correction will be the \"obsolete memoization pattern\", which can further be corrected by the `Sorbet/ObsoleteStrictMemoization` cop.
See `Sorbet/ObsoleteStrictMemoization` for more details.",
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
