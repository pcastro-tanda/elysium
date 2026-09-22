//! `Style/TrailingCommaInHashLiteral`, ported from RuboCop's `lib/rubocop/cop/style/trailing_comma_in_hash_literal.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Style/TrailingCommaInHashLiteral`.
#[derive(Debug, Clone)]
pub struct TrailingCommaInHashLiteral;

impl Rule for TrailingCommaInHashLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Style/TrailingCommaInHashLiteral",
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
