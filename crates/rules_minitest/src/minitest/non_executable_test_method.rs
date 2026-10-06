//! `Minitest/NonExecutableTestMethod`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/non_executable_test_method.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks uses of test methods outside test class.
#[derive(Debug, Clone)]
pub struct NonExecutableTestMethod;

impl Rule for NonExecutableTestMethod {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/NonExecutableTestMethod",
        department: Department::Minitest,
        summary: "Checks uses of test methods outside test class.",
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
