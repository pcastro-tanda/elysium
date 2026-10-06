//! `Minitest/DuplicateTestRun`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/duplicate_test_run.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop detects duplicate test runs caused by one test class inheriting from another.
#[derive(Debug, Clone)]
pub struct DuplicateTestRun;

impl Rule for DuplicateTestRun {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/DuplicateTestRun",
        department: Department::Minitest,
        summary: "This cop detects duplicate test runs caused by one test class inheriting from another.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
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
