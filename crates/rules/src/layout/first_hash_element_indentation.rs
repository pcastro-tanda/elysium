//! `Layout/FirstHashElementIndentation`, ported from RuboCop's `lib/rubocop/cop/layout/first_hash_element_indentation.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Layout/FirstHashElementIndentation`.
#[derive(Debug, Clone)]
pub struct FirstHashElementIndentation;

impl Rule for FirstHashElementIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/FirstHashElementIndentation",
        department: Department::Layout,
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
