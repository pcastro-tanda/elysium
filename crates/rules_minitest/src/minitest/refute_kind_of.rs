//! `Minitest/RefuteKindOf`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_kind_of.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the test to use `refute_kind_of(Class, object)` over `refute(object.kind_of?(Class))`.
#[derive(Debug, Clone)]
pub struct RefuteKindOf;

impl Rule for RefuteKindOf {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteKindOf",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `refute_kind_of(Class, object)` over `refute(object.kind_of?(Class))`.",
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
