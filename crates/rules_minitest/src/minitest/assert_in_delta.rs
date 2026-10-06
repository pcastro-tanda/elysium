//! `Minitest/AssertInDelta`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_in_delta.rb` (with `InDeltaMixin`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// Enforces the test to use `assert_in_delta` instead of using `assert_equal` to compare floats.
#[derive(Debug, Clone)]
pub struct AssertInDelta;

impl Rule for AssertInDelta {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertInDelta",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `assert_in_delta` instead of using `assert_equal` to compare floats.",
        explanation: "Enforces the test to use `assert_in_delta` instead of using `assert_equal` to compare floats.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // `(send nil? :assert_equal $_ $_ $...)`
        if call.receiver().is_some()
            || call.is_safe_navigation()
            || call.name().as_slice() != b"assert_equal"
        {
            return;
        }
        let arguments = argument_list(&call);
        let [expected, actual, rest @ ..] = arguments.as_slice() else { return };
        if expected.as_float_node().is_none() && actual.as_float_node().is_none() {
            return;
        }
        let source = |node: &Node<'_>| String::from_utf8_lossy(ctx.text(node.span())).into_owned();
        let good_method = match rest.first() {
            Some(message) => format!(
                "assert_in_delta({}, {}, 0.001, {})",
                source(expected),
                source(actual),
                source(message)
            ),
            None => format!("assert_in_delta({}, {})", source(expected), source(actual)),
        };
        let message = format!("Prefer using `{good_method}`.");
        let span = call_span_excluding_block(&call);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, good_method.into_bytes())],
            },
        );
    }
}

/// The call's arguments the way whitequark lists a `send`'s: a `&block`
/// argument is one of them.
fn argument_list<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut args: Vec<Node<'pr>> =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block().filter(|block| block.as_block_argument_node().is_some()) {
        args.push(block);
    }
    args
}
