//! `Layout/SpaceInsideArrayLiteralBrackets`, ported from RuboCop's `lib/rubocop/cop/layout/space_inside_array_literal_brackets.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Layout/SpaceInsideArrayLiteralBrackets`.
#[derive(Debug, Clone)]
pub struct SpaceInsideArrayLiteralBrackets;

impl Rule for SpaceInsideArrayLiteralBrackets {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceInsideArrayLiteralBrackets",
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
