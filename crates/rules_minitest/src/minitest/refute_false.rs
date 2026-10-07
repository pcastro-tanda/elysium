//! `Minitest/RefuteFalse`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_false.rb` (with its `ArgumentRangeHelper` mixin).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Enforces the use of `refute(object)` over `assert_equal(false, object)`.
#[derive(Debug, Clone)]
pub struct RefuteFalse;

impl Rule for RefuteFalse {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteFalse",
        department: Department::Minitest,
        summary: "Check if your test uses `refute(actual)` instead of \
                  `assert_equal(false, actual)`.",
        explanation: "Enforces the use of `refute(object)` over `assert_equal(false, object)`.\n\n\
                      This cop is unsafe because it cannot detect failure when second argument \
                      is `nil`. False positives cannot be prevented when this is a variable or \
                      method return value.\n\n```ruby\n# bad\nassert_equal(false, actual)\n\
                      assert_equal(false, actual, 'message')\n\nassert(!test)\n\
                      assert(!test, 'message')\n\n# good\nrefute(actual)\n\
                      refute(actual, 'message')\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
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
        if call.receiver().is_some() {
            return;
        }
        let arguments = argument_list(&call);
        let is_assert_equal = call.name().as_slice() == b"assert_equal";
        let (actual, rest_index) = match call.name().as_slice() {
            b"assert_equal" => {
                // (send nil? :assert_equal false $_ $...)
                let [expected, actual, ..] = arguments.as_slice() else { return };
                if expected.as_false_node().is_none() {
                    return;
                }
                (*actual, 2)
            }
            b"assert" => {
                // (send nil? :assert (send $_ :!) $...)
                let Some(bang) = arguments.first().and_then(Node::as_call_node) else { return };
                if bang.name().as_slice() != b"!"
                    || bang.is_safe_navigation()
                    || bang.arguments().is_some()
                    || bang.block().is_some()
                {
                    return;
                }
                let Some(actual) = bang.receiver() else { return };
                (actual, 1)
            }
            _ => return,
        };

        let actual_source = String::from_utf8_lossy(ctx.text(actual.span())).into_owned();
        let args = match arguments.get(rest_index) {
            Some(message) => {
                format!("{actual_source}, {}", String::from_utf8_lossy(ctx.text(message.span())))
            }
            None => actual_source.clone(),
        };
        let message = format!("Prefer using `refute({args})`.");

        let Some(selector) = call.message_loc() else { return };
        let mut edits = vec![Edit::replace(selector.span(), b"refute".to_vec())];
        if is_assert_equal {
            // `first_and_second_arguments_range`.
            let range = Span::new(arguments[0].span().start, arguments[1].span().end);
            edits.push(Edit::replace(range, actual_source.into_bytes()));
        } else {
            edits.push(Edit::replace(arguments[0].span(), actual_source.into_bytes()));
        }
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
