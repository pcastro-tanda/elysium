//! `Minitest/UnreachableAssertion`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/unreachable_assertion.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop checks for an `assert_raises` block containing any unreachable assertions.
#[derive(Debug, Clone)]
pub struct UnreachableAssertion;

impl Rule for UnreachableAssertion {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/UnreachableAssertion",
        department: Department::Minitest,
        summary: "This cop checks for an `assert_raises` block containing any unreachable assertions.",
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
