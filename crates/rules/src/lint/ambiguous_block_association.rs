//! `Lint/AmbiguousBlockAssociation`, ported from RuboCop's `lib/rubocop/cop/lint/ambiguous_block_association.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Lint/AmbiguousBlockAssociation`.
#[derive(Debug, Clone)]
pub struct AmbiguousBlockAssociation;

impl Rule for AmbiguousBlockAssociation {
    const META: RuleMeta = RuleMeta {
        name: "Lint/AmbiguousBlockAssociation",
        department: Department::Lint,
        summary: "TODO",
        explanation: "TODO",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }
}
