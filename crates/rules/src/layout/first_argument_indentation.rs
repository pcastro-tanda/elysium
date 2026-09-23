//! `Layout/FirstArgumentIndentation`, ported from RuboCop's `lib/rubocop/cop/layout/first_argument_indentation.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Layout/FirstArgumentIndentation`.
#[derive(Debug, Clone)]
pub struct FirstArgumentIndentation;

impl Rule for FirstArgumentIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/FirstArgumentIndentation",
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
