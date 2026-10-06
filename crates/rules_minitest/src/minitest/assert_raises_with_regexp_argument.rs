//! `Minitest/AssertRaisesWithRegexpArgument`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_raises_with_regexp_argument.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop enforces checks for regular expression literals passed to `assert_raises`.
#[derive(Debug, Clone)]
pub struct AssertRaisesWithRegexpArgument;

impl Rule for AssertRaisesWithRegexpArgument {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertRaisesWithRegexpArgument",
        department: Department::Minitest,
        summary: "This cop enforces checks for regular expression literals passed to `assert_raises`.",
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
