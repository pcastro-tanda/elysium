//! `Style/SoleNestedConditional`, ported from RuboCop's `lib/rubocop/cop/style/sole_nested_conditional.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Style/SoleNestedConditional`.
#[derive(Debug, Clone)]
pub struct SoleNestedConditional;

impl Rule for SoleNestedConditional {
    const META: RuleMeta = RuleMeta {
        name: "Style/SoleNestedConditional",
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
