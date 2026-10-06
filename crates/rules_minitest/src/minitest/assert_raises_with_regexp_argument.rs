//! `Minitest/AssertRaisesWithRegexpArgument`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_raises_with_regexp_argument.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{Node, NodeKind};

const MSG: &str =
    "Do not pass regular expression literals to `assert_raises`. Test the resulting exception.";

/// This cop enforces checks for regular expression literals passed to `assert_raises`.
#[derive(Debug, Clone)]
pub struct AssertRaisesWithRegexpArgument;

impl Rule for AssertRaisesWithRegexpArgument {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertRaisesWithRegexpArgument",
        department: Department::Minitest,
        summary:
            "This cop enforces checks for regular expression literals passed to `assert_raises`.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() || call.name().as_slice() != b"assert_raises" {
            return;
        }
        // `last_argument`: a `&block` argument is the last one in whitequark.
        if call.block().is_some_and(|block| block.as_block_argument_node().is_some()) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let Some(last) = arguments.arguments().iter().last() else { return };
        if last.as_regular_expression_node().is_some()
            || last.as_interpolated_regular_expression_node().is_some()
        {
            ctx.report(&Self::META, call_span_excluding_block(&call), MSG);
        }
    }
}
