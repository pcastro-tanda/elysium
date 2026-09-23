//! `Layout/ExtraSpacing`, ported from RuboCop's `lib/rubocop/cop/layout/extra_spacing.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Layout/ExtraSpacing`.
#[derive(Debug, Clone)]
pub struct ExtraSpacing;

impl Rule for ExtraSpacing {
    const META: RuleMeta = RuleMeta {
        name: "Layout/ExtraSpacing",
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
