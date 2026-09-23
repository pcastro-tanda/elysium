//! `Style/RedundantRegexpCharacterClass`, ported from RuboCop's `lib/rubocop/cop/style/redundant_regexp_character_class.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Style/RedundantRegexpCharacterClass`.
#[derive(Debug, Clone)]
pub struct RedundantRegexpCharacterClass;

impl Rule for RedundantRegexpCharacterClass {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantRegexpCharacterClass",
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
