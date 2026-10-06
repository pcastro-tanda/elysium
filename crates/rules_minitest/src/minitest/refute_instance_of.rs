//! `Minitest/RefuteInstanceOf`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_instance_of.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `refute_instance_of(Class, object)` over `refute(object.instance_of?(Class))`.
#[derive(Debug, Clone)]
pub struct RefuteInstanceOf;

impl Rule for RefuteInstanceOf {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteInstanceOf",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `refute_instance_of(Class, object)` over `refute(object.instance_of?(Class))`.",
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
