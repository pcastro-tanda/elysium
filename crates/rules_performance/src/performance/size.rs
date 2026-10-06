//! `Performance/Size`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/size.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `size` instead of `count` for counting the number of elements in `Array` and `Hash`.
#[derive(Debug, Clone)]
pub struct Size;

impl Rule for Size {
    const META: RuleMeta = RuleMeta {
        name: "Performance/Size",
        department: Department::Performance,
        summary: "Use `size` instead of `count` for counting the number of elements in `Array` and `Hash`.",
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
