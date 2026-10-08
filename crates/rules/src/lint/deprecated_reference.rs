//! `Lint/DeprecatedReference`, ported from RuboCop's
//! `lib/rubocop/cop/lint/deprecated_reference.rb`.
//!
//! Entirely powered by RuboCop's project-wide index (`ProjectIndexHelp`),
//! unavailable in this engine, so this cop never has anything to report --
//! matching its own single fixture (`does_not_register_an_offense_without_a_project_index`).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks for references to methods and constants documented as deprecated with a YARD `@deprecated` tag. Requires `AllCops/UseProjectIndex` to be enabled.
#[derive(Debug, Clone)]
pub struct DeprecatedReference;

impl Rule for DeprecatedReference {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DeprecatedReference",
        department: Department::Lint,
        summary: "Checks for references to methods and constants documented as deprecated with a YARD `@deprecated` tag. Requires `AllCops/UseProjectIndex` to be enabled.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[],
        blind_spots: "No project-wide index is available (no `rubydex` gem equivalent), so this \
cop never reports anything; it is a documented no-op, matching upstream's own \"without the index \
the cop does nothing\" behavior.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let _ = (node, ctx, NodeKind::CallNode);
    }
}
