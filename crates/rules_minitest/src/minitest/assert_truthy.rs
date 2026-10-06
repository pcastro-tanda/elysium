//! `Minitest/AssertTruthy`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_truthy.rb` (with its `ArgumentRangeHelper` mixin).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Enforces the test to use `assert(actual)` instead of using `assert_equal(true, actual)`.
#[derive(Debug, Clone)]
pub struct AssertTruthy;

impl Rule for AssertTruthy {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertTruthy",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `assert(actual)` instead of using \
                  `assert_equal(true, actual)`.",
        explanation: "Enforces the test to use `assert(actual)` instead of using \
                      `assert_equal(true, actual)`.\n\nThis cop is unsafe because true might be \
                      expected instead of truthy. False positives cannot be prevented when this \
                      is a variable or method return value.\n\n```ruby\n# bad\n\
                      assert_equal(true, actual)\nassert_equal(true, actual, 'message')\n\n\
                      # good\nassert(actual)\nassert(actual, 'message')\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
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
        if call.receiver().is_some() || call.name().as_slice() != b"assert_equal" {
            return;
        }
        let arguments = argument_list(&call);
        // (send nil? :assert_equal true $_ $...)
        let [expected, actual, rest @ ..] = arguments.as_slice() else { return };
        if expected.as_true_node().is_none() {
            return;
        }

        let actual_source = String::from_utf8_lossy(ctx.text(actual.span())).into_owned();
        let args = match rest.first() {
            Some(message) => {
                format!("{actual_source}, {}", String::from_utf8_lossy(ctx.text(message.span())))
            }
            None => actual_source.clone(),
        };
        let message = format!("Prefer using `assert({args})`.");

        let Some(selector) = call.message_loc() else { return };
        // `first_and_second_arguments_range`.
        let range = Span::new(expected.span().start, actual.span().end);
        let edits = vec![
            Edit::replace(selector.span(), b"assert".to_vec()),
            Edit::replace(range, actual_source.into_bytes()),
        ];
        ctx.report_with_fix(
            &Self::META,
            call_span_excluding_block(&call),
            message,
            Fix { applicability: Applicability::Unsafe, edits },
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
