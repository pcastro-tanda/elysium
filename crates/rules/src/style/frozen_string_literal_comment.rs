//! `Style/FrozenStringLiteralComment`, ported from RuboCop's `lib/rubocop/cop/style/frozen_string_literal_comment.rb`.
//!
//! Stub: registered so the rule set compiles; the implementation lands with
//! its fixtures.

use linter::{
    Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};

/// `Style/FrozenStringLiteralComment`.
#[derive(Debug, Clone)]
pub struct FrozenStringLiteralComment;

impl Rule for FrozenStringLiteralComment {
    const META: RuleMeta = RuleMeta {
        name: "Style/FrozenStringLiteralComment",
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
