//! `Style/IfUnlessModifierOfIfUnless`, ported from RuboCop's `lib/rubocop/cop/style/if_unless_modifier_of_if_unless.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Style/IfUnlessModifierOfIfUnless`.
#[derive(Debug, Clone)]
pub struct IfUnlessModifierOfIfUnless;

impl Rule for IfUnlessModifierOfIfUnless {
    const META: RuleMeta = RuleMeta {
        name: "Style/IfUnlessModifierOfIfUnless",
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
