//! `Minitest/RefuteEqual`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_equal.rb` (with its `ArgumentRangeHelper` mixin).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_PREFIX: &str = "Prefer using `refute_equal(";

/// Enforces the use of `refute_equal(expected, object)` over `assert(expected != actual)` or `assert(! expected == actual)`.
#[derive(Debug, Clone)]
pub struct RefuteEqual;

impl Rule for RefuteEqual {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteEqual",
        department: Department::Minitest,
        summary: "Enforces the use of `refute_equal(expected, object)` over `assert(expected != actual)` or `assert(! expected == actual)`.",
        explanation: "Enforces the use of `refute_equal(expected, object)` over `assert(expected != actual)` or `assert(! expected == actual)`.\n\n```ruby\n# bad\nassert(\"rubocop-minitest\" != actual)\nrefute(\"rubocop-minitest\" == actual)\nassert_operator(\"rubocop-minitest\", :!=, actual)\nrefute_operator(\"rubocop-minitest\", :==, actual)\n\n# good\nrefute_equal(\"rubocop-minitest\", actual)\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
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
        if call.receiver().is_some() || call.is_safe_navigation() {
            return;
        }
        let arguments = argument_list(&call);
        let Some(found) = matcher(&call, &arguments) else { return };

        let source = |node: &Node<'_>| String::from_utf8_lossy(ctx.text(node.span())).into_owned();
        let basic_arguments = format!("{}, {}", source(&found.expected), source(&found.actual));
        let preferred = match arguments.get(found.rest) {
            Some(message_arg) => format!("{basic_arguments}, {}", source(message_arg)),
            None => basic_arguments.clone(),
        };
        let message = format!("{MSG_PREFIX}{preferred})`.");

        let Some(selector) = call.message_loc() else { return };
        let name = call.name();
        let name = name.as_slice();
        let range = if name == b"assert" || name == b"refute" {
            arguments[0].span()
        } else {
            Span::new(arguments[0].span().start, arguments[2].span().end)
        };
        ctx.report_with_fix(
            &Self::META,
            call_span_excluding_block(&call),
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::replace(selector.span(), b"refute_equal".to_vec()),
                    Edit::replace(range, basic_arguments.into_bytes()),
                ],
            },
        );
    }
}

struct Match<'pr> {
    expected: Node<'pr>,
    actual: Node<'pr>,
    /// Index of the first of the `$...` rest arguments.
    rest: usize,
}

/// The node pattern:
///
/// ```text
/// {
/// (send nil? :assert (send $_ :!= $_) $...)
/// (send nil? :refute (send $_ :== $_) $...)
/// (send nil? :assert_operator $_ (sym :!=) $_ $...)
/// (send nil? :refute_operator $_ (sym :==) $_ $...)
/// }
/// ```
fn matcher<'pr>(call: &CallNode<'pr>, arguments: &[Node<'pr>]) -> Option<Match<'pr>> {
    match call.name().as_slice() {
        b"assert" => {
            // `(send nil? :assert (send $_ :!= $_) $...)`
            let inner = arguments.first()?.as_call_node()?;
            if inner.name().as_slice() != b"!=" || !is_plain_send(&inner) {
                return None;
            }
            let inner_args = argument_list(&inner);
            let [actual] = inner_args.as_slice() else { return None };
            Some(Match { expected: inner.receiver()?, actual: *actual, rest: 1 })
        }
        b"refute" => {
            // `(send nil? :refute (send $_ :== $_) $...)`
            let inner = arguments.first()?.as_call_node()?;
            if inner.name().as_slice() != b"==" || !is_plain_send(&inner) {
                return None;
            }
            let inner_args = argument_list(&inner);
            let [actual] = inner_args.as_slice() else { return None };
            Some(Match { expected: inner.receiver()?, actual: *actual, rest: 1 })
        }
        b"assert_operator" => {
            // `(send nil? :assert_operator $_ (sym :!=) $_ $...)`
            let [expected, operator, actual, ..] = arguments else { return None };
            if operator.as_symbol_node()?.unescaped() != b"!=" {
                return None;
            }
            Some(Match { expected: *expected, actual: *actual, rest: 3 })
        }
        b"refute_operator" => {
            // `(send nil? :refute_operator $_ (sym :==) $_ $...)`
            let [expected, operator, actual, ..] = arguments else { return None };
            if operator.as_symbol_node()?.unescaped() != b"==" {
                return None;
            }
            Some(Match { expected: *expected, actual: *actual, rest: 3 })
        }
        _ => None,
    }
}

/// A `send` (not `csend`, not wrapped in a `block`).
fn is_plain_send(call: &CallNode<'_>) -> bool {
    !call.is_safe_navigation()
        && call.block().is_none_or(|block| block.as_block_argument_node().is_some())
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
