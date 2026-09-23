//! `Lint/Debugger`, ported from RuboCop's `lib/rubocop/cop/lint/debugger.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Lint/Debugger`.
#[derive(Debug, Clone)]
pub struct Debugger;

impl Rule for Debugger {
    const META: RuleMeta = RuleMeta {
        name: "Lint/Debugger",
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
