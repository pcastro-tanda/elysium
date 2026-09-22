//! `Style/TrailingCommaInArrayLiteral`, ported from RuboCop's `lib/rubocop/cop/style/trailing_comma_in_array_literal.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Style/TrailingCommaInArrayLiteral`.
#[derive(Debug, Clone)]
pub struct TrailingCommaInArrayLiteral;

impl Rule for TrailingCommaInArrayLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Style/TrailingCommaInArrayLiteral",
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
