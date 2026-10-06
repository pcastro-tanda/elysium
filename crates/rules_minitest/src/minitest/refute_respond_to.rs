//! `Minitest/RefuteRespondTo`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_respond_to.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `refute_respond_to(object, :do_something)` over `refute(object.respond_to?(:do_something))`.
#[derive(Debug, Clone)]
pub struct RefuteRespondTo;

impl Rule for RefuteRespondTo {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteRespondTo",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `refute_respond_to(object, :do_something)` over `refute(object.respond_to?(:do_something))`.",
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
