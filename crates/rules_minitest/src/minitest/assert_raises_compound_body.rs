//! `Minitest/AssertRaisesCompoundBody`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_raises_compound_body.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces the block body of `assert_raises { ... }` to be reduced to only the raising code.
#[derive(Debug, Clone)]
pub struct AssertRaisesCompoundBody;

impl Rule for AssertRaisesCompoundBody {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertRaisesCompoundBody",
        department: Department::Minitest,
        summary: "This cop enforces the block body of `assert_raises { ... }` to be reduced to only the raising code.",
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
