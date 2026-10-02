//! `Lint/ArgumentMismatch`, ported from RuboCop's
//! `lib/rubocop/cop/lint/argument_mismatch.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks for calls that pass the wrong number of positional arguments to a method, using the project index.
#[derive(Debug, Clone)]
pub struct ArgumentMismatch;

impl Rule for ArgumentMismatch {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ArgumentMismatch",
        department: Department::Lint,
        summary: "Checks for calls that pass the wrong number of positional arguments to a method, using the project index.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let _ = (node, ctx, NodeKind::CallNode);
    }
}
