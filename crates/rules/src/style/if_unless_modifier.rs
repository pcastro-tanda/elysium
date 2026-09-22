//! `Style/IfUnlessModifier`, ported from RuboCop's `lib/rubocop/cop/style/if_unless_modifier.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Style/IfUnlessModifier`.
#[derive(Debug, Clone)]
pub struct IfUnlessModifier;

impl Rule for IfUnlessModifier {
    const META: RuleMeta = RuleMeta {
        name: "Style/IfUnlessModifier",
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
