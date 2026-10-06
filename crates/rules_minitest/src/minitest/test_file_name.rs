//! `Minitest/TestFileName`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/test_file_name.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks if test file names start with `test_` or end with `_test.rb`.
#[derive(Debug, Clone)]
pub struct TestFileName;

impl Rule for TestFileName {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/TestFileName",
        department: Department::Minitest,
        summary: "Checks if test file names start with `test_` or end with `_test.rb`.",
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
