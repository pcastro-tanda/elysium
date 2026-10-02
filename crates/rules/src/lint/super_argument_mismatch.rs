//! `Lint/SuperArgumentMismatch`, ported from RuboCop's
//! `lib/rubocop/cop/lint/super_argument_mismatch.rb`.
//!
//! Entirely powered by `AllCops/UseProjectIndex` and the `rubydex` gem's
//! project-wide index (`ProjectIndexHelp`, `IndexedMethodArity`): without
//! the index the cop does nothing, per upstream's own doc. elysium has no
//! such index (see `lint/deprecated_reference.rs`, `lint/duplicate_methods.rs`,
//! `naming/predicate_prefix.rs` for the same gap), so this is a no-op,
//! matching upstream's own "without a project index" behavior exactly --
//! the only fixture case.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks for `super` calls that pass the wrong number of positional arguments to the overridden implementation, using the project index.
#[derive(Debug, Clone)]
pub struct SuperArgumentMismatch;

impl Rule for SuperArgumentMismatch {
    const META: RuleMeta = RuleMeta {
        name: "Lint/SuperArgumentMismatch",
        department: Department::Lint,
        summary: "Checks for `super` calls that pass the wrong number of positional arguments to the overridden implementation, using the project index.",
        explanation: "\
Checks for `super` calls with explicit arguments that pass the wrong
number of positional arguments to the overridden implementation, using
the project index.

The check is powered by the project-wide index, so it only runs when
`AllCops/UseProjectIndex` is enabled and the `rubydex` gem is installed.
Without the index the cop does nothing.",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[],
        blind_spots: "\
Not ported: `AllCops/UseProjectIndex` cross-file `super` arity checking \
(`ProjectIndexHelp`, `IndexedMethodArity`, method-resolution-order lookup) \
needs the `rubydex` gem's project index, which elysium has no equivalent of. \
This cop is always a no-op here, matching upstream's own documented behavior \
when the index is unavailable.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let _ = (node, ctx, NodeKind::CallNode);
    }
}
