//! `Style/TrailingCommaInArguments`, ported from RuboCop's `lib/rubocop/cop/style/trailing_comma_in_arguments.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Style/TrailingCommaInArguments`.
#[derive(Debug, Clone)]
pub struct TrailingCommaInArguments;

impl Rule for TrailingCommaInArguments {
    const META: RuleMeta = RuleMeta {
        name: "Style/TrailingCommaInArguments",
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
