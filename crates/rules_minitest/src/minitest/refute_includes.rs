//! `Minitest/RefuteIncludes`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_includes.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const ASSERTION_TYPE: &str = "refute";

/// This cop enforces the test to use `refute_includes` instead of using `refute(collection.include?(object))`.
#[derive(Debug, Clone)]
pub struct RefuteIncludes;

impl Rule for RefuteIncludes {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteIncludes",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `refute_includes` instead of using `refute(collection.include?(object))`.",
        explanation: "",
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
        if call.is_safe_navigation() || call.name().as_slice() != ASSERTION_TYPE.as_bytes() {
            return;
        }
        let arguments = argument_list(&call);
        let Some(first) = arguments.first().and_then(Node::as_call_node) else {
            return;
        };
        if first.block().is_some_and(|block| block.as_block_node().is_some()) {
            return;
        }
        let inner_arguments = argument_list(&first);
        if inner_arguments.is_empty()
            || !matches!(first.name().as_slice(), b"include?" | b"member?")
        {
            return;
        }
        let receiver = match first.receiver() {
            Some(receiver) => String::from_utf8_lossy(ctx.text(receiver.span())).into_owned(),
            None => "self".to_string(),
        };
        let mut new_arguments = vec![receiver];
        if let Some(method_argument) = inner_arguments.first() {
            new_arguments
                .push(String::from_utf8_lossy(ctx.text(method_argument.span())).into_owned());
        }
        let new_arguments = new_arguments.join(", ");

        // `message_argument = arguments.last if arguments.first != arguments.last`
        let message_argument = match (arguments.first(), arguments.last()) {
            (Some(first), Some(last)) if ctx.text(first.span()) != ctx.text(last.span()) => {
                Some(String::from_utf8_lossy(ctx.text(last.span())).into_owned())
            }
            _ => None,
        };
        let offered = match &message_argument {
            Some(message) => format!("{new_arguments}, {message}"),
            None => new_arguments.clone(),
        };
        let message = format!("Prefer using `refute_includes({offered})`.");

        let Some(selector) = call.message_loc() else { return };
        ctx.report_with_fix(
            &Self::META,
            call_span_excluding_block(&call),
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::replace(selector.span(), b"refute_includes".to_vec()),
                    Edit::replace(arguments[0].span(), new_arguments.into_bytes()),
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
