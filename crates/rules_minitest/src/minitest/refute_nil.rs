//! `Minitest/RefuteNil`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_nil.rb` (with its
//! `NilAssertionHandleable` and `ArgumentRangeHelper` mixins).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const ASSERTION_TYPE: &str = "refute";

/// This cop enforces the test to use `refute_nil` instead of using
/// `refute_equal(nil, something)` or `refute(something.nil?)`.
#[derive(Debug, Clone)]
pub struct RefuteNil;

impl Rule for RefuteNil {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteNil",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `refute_nil` instead of using \
                  `refute_equal(nil, something)` or `refute(something.nil?)`.",
        explanation: "Enforces the test to use `refute_nil` instead of using \
                      `refute_equal(nil, something)`, `refute(something.nil?)`, or \
                      `refute_predicate(something, :nil?)`.\n\n```ruby\n# bad\n\
                      refute_equal(nil, actual)\nrefute_equal(nil, actual, 'message')\n\
                      refute(object.nil?)\nrefute(object.nil?, 'message')\n\
                      refute_predicate(object, :nil?)\n\
                      refute_predicate(object, :nil?, 'message')\n\n# good\n\
                      refute_nil(actual)\nrefute_nil(actual, 'message')\n```",
        enabled_by_default: true,
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
        if call.receiver().is_some() {
            return;
        }
        let arguments = argument_list(&call);
        let Some(assertion) = nil_assertion(&call, &arguments) else { return };

        let actual_source = String::from_utf8_lossy(ctx.text(assertion.actual.span())).into_owned();
        let preferred_args = match arguments.get(assertion.message_index) {
            Some(message) => {
                format!("{actual_source}, {}", String::from_utf8_lossy(ctx.text(message.span())))
            }
            None => actual_source.clone(),
        };
        let message = format!("Prefer using `{ASSERTION_TYPE}_nil({preferred_args})`.");

        let Some(selector) = call.message_loc() else { return };
        let mut edits =
            vec![Edit::replace(selector.span(), format!("{ASSERTION_TYPE}_nil").into_bytes())];
        if let Some(predicate) = assertion.predicate {
            // `refute(actual.nil?)`: drop the `.nil?`.
            if let Some(dot) = predicate.call_operator_loc() {
                edits.push(Edit::delete(dot.span()));
            }
            if let Some(nil_selector) = predicate.message_loc() {
                edits.push(Edit::delete(nil_selector.span()));
            }
        } else {
            // `first_and_second_arguments_range`, replaced by `actual`.
            let range = Span::new(arguments[0].span().start, arguments[1].span().end);
            edits.push(Edit::replace(range, actual_source.into_bytes()));
        }
        ctx.report_with_fix(
            &Self::META,
            call_span_excluding_block(&call),
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// One match of `nil_assertion`.
struct NilAssertion<'pr> {
    /// `$_`: what the assertion checks for `nil`.
    actual: Node<'pr>,
    /// The `actual.nil?` call of the `refute` form.
    predicate: Option<CallNode<'pr>>,
    /// Where the message arguments (`$...`) start.
    message_index: usize,
}

/// The `nil_assertion` node pattern:
///
/// ```text
/// (send nil? :refute_equal nil $_ $...)
/// (send nil? :refute (send $_ :nil?) $...)
/// (send nil? :refute_predicate $_ (sym :nil?) $...)
/// ```
fn nil_assertion<'pr>(call: &CallNode<'pr>, arguments: &[Node<'pr>]) -> Option<NilAssertion<'pr>> {
    match call.name().as_slice() {
        b"refute_equal" => {
            let [expected, actual, ..] = arguments else { return None };
            expected.as_nil_node()?;
            Some(NilAssertion { actual: *actual, predicate: None, message_index: 2 })
        }
        b"refute" => {
            let predicate = arguments.first()?.as_call_node()?;
            if predicate.name().as_slice() != b"nil?"
                || predicate.is_safe_navigation()
                || predicate.arguments().is_some()
                || predicate.block().is_some()
            {
                return None;
            }
            let actual = predicate.receiver()?;
            Some(NilAssertion { actual, predicate: Some(predicate), message_index: 1 })
        }
        b"refute_predicate" => {
            let [actual, predicate, ..] = arguments else { return None };
            if predicate.as_symbol_node()?.unescaped() != b"nil?" {
                return None;
            }
            Some(NilAssertion { actual: *actual, predicate: None, message_index: 2 })
        }
        _ => None,
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
