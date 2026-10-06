//! `Minitest/AssertOperator`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_operator.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const ALLOWED_OPERATORS: [&[u8]; 1] = [b"[]"];

/// `MethodIdentifierPredicates::OPERATOR_METHODS`.
const OPERATOR_METHODS: [&[u8]; 29] = [
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~",
];

/// Enforces the use of `assert_operator(expected, :<, actual)` over `assert(expected < actual)`.
#[derive(Debug, Clone)]
pub struct AssertOperator;

impl Rule for AssertOperator {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertOperator",
        department: Department::Minitest,
        summary: "Enforces the use of `assert_operator(expected, :<, actual)` over `assert(expected < actual)`.",
        explanation: "Enforces the use of `assert_operator(expected, :<, actual)` over `assert(expected < actual)`.\n\n```ruby\n# bad\nassert(expected < actual)\n\n# good\nassert_operator(expected, :<, actual)\n```",
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
        if call.is_safe_navigation() || call.name().as_slice() != b"assert" {
            return;
        }
        let arguments = argument_list(&call);
        let Some(first_argument) = arguments.first() else { return };
        // `first_argument.respond_to?(:binary_operation?) && binary_operation?`
        let Some(operation) = first_argument.as_call_node() else { return };
        if operation.block().is_some_and(|block| block.as_block_argument_node().is_none()) {
            return;
        }
        let operation_name = operation.name();
        let operation_name = operation_name.as_slice();
        let Some(selector) = operation.message_loc() else { return };
        if !OPERATOR_METHODS.contains(&operation_name)
            || selector.span().start == operation.location().span().start
        {
            return;
        }
        if ALLOWED_OPERATORS.contains(&operation_name) {
            return;
        }
        // `lhs, op, rhs = *node.first_argument`
        let (Some(lhs), operation_arguments) = (operation.receiver(), argument_list(&operation))
        else {
            return;
        };
        let Some(rhs) = operation_arguments.first() else { return };

        let source = |node: &Node<'_>| String::from_utf8_lossy(ctx.text(node.span())).into_owned();
        let mut new_arguments = format!(
            "{}, :{}, {}",
            source(&lhs),
            String::from_utf8_lossy(operation_name),
            source(rhs)
        );
        if arguments.len() == 2 {
            new_arguments.push_str(", ");
            new_arguments.push_str(&source(&arguments[1]));
        }
        let message = format!("Prefer using `assert_operator({new_arguments})`.");

        let Some(call_selector) = call.message_loc() else { return };
        let range = Span::new(
            first_argument.span().start,
            arguments.last().map_or(first_argument.span().end, |last| last.span().end),
        );
        ctx.report_with_fix(
            &Self::META,
            call_span_excluding_block(&call),
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::replace(call_selector.span(), b"assert_operator".to_vec()),
                    Edit::replace(range, new_arguments.into_bytes()),
                ],
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
