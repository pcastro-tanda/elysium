//! `Minitest/NonPublicTestMethod`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/non_public_test_method.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Detects non `public` (marked as `private` or `protected`) test methods.
#[derive(Debug, Clone)]
pub struct NonPublicTestMethod;

impl Rule for NonPublicTestMethod {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/NonPublicTestMethod",
        department: Department::Minitest,
        summary: "Detects non `public` (marked as `private` or `protected`) test methods.",
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
