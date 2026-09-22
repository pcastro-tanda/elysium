//! `Style/GuardClause`, ported from RuboCop's `lib/rubocop/cop/style/guard_clause.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Style/GuardClause`.
#[derive(Debug, Clone)]
pub struct GuardClause;

impl Rule for GuardClause {
    const META: RuleMeta = RuleMeta {
        name: "Style/GuardClause",
        department: Department::Style,
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
