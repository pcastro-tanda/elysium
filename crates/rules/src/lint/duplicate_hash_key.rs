//! `Lint/DuplicateHashKey`, ported from RuboCop's `lib/rubocop/cop/lint/duplicate_hash_key.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Lint/DuplicateHashKey`.
#[derive(Debug, Clone)]
pub struct DuplicateHashKey;

impl Rule for DuplicateHashKey {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DuplicateHashKey",
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
