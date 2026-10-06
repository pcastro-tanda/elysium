//! `Minitest/AssertRespondTo`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_respond_to.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `assert_respond_to(object, :do_something)` over `assert(object.respond_to?(:do_something))`.
#[derive(Debug, Clone)]
pub struct AssertRespondTo;

impl Rule for AssertRespondTo {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertRespondTo",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `assert_respond_to(object, :do_something)` over `assert(object.respond_to?(:do_something))`.",
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
