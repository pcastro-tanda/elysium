//! `Minitest/RefuteEmpty`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_empty.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces to use `refute_empty` instead of using `refute(object.empty?)`.
#[derive(Debug, Clone)]
pub struct RefuteEmpty;

impl Rule for RefuteEmpty {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteEmpty",
        department: Department::Minitest,
        summary: "This cop enforces to use `refute_empty` instead of using `refute(object.empty?)`.",
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
