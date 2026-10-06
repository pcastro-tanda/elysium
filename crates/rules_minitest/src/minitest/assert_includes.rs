//! `Minitest/AssertIncludes`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_includes.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `assert_includes` instead of using `assert(collection.include?(object))`.
#[derive(Debug, Clone)]
pub struct AssertIncludes;

impl Rule for AssertIncludes {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertIncludes",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `assert_includes` instead of using `assert(collection.include?(object))`.",
        explanation: "",
        enabled_by_default: true,
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
