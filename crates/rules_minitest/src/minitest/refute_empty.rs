//! `Minitest/RefuteEmpty`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_empty.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const ASSERTION_TYPE: &str = "refute";

/// This cop enforces to use `refute_empty` instead of using `refute(object.empty?)`.
#[derive(Debug, Clone)]
pub struct RefuteEmpty;

impl Rule for RefuteEmpty {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteEmpty",
        department: Department::Minitest,
        summary:
            "This cop enforces to use `refute_empty` instead of using `refute(object.empty?)`.",
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
        let Some(first_argument) = arguments.first() else { return };
        // `node.first_argument.method?(:empty?)` and `.arguments.empty?` hold for
        // a call, a block (its own parameters) and a `def` alike.
        let Some(receiver) = empty_receiver(first_argument) else { return };
        let receiver = match receiver {
            Some(receiver) => String::from_utf8_lossy(ctx.text(receiver.span())).into_owned(),
            None => "self".to_string(),
        };
        let new_arguments = receiver;

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
        let message = format!("Prefer using `refute_empty({offered})`.");

        let Some(selector) = call.message_loc() else { return };
        ctx.report_with_fix(
            &Self::META,
            call_span_excluding_block(&call),
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::replace(selector.span(), b"refute_empty".to_vec()),
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

/// `first_argument.method?(:empty?) && first_argument.arguments.empty?` for the
/// node types rubocop-ast gives `method?`: `send`/`csend`, `block` (and
/// `numblock`/`itblock`, whose arguments are the block's own parameters) and
/// `def`/`defs`. The outer `Some` is the match; the inner is the receiver.
#[allow(clippy::option_option)]
fn empty_receiver<'pr>(node: &Node<'pr>) -> Option<Option<Node<'pr>>> {
    if let Some(call) = node.as_call_node() {
        if call.name().as_slice() != b"empty?" {
            return None;
        }
        return match call.block().and_then(|block| block.as_block_node()) {
            Some(block) => block_parameters_empty(&block).then(|| call.receiver()),
            None => argument_list(&call).is_empty().then(|| call.receiver()),
        };
    }
    let def = node.as_def_node()?;
    if def.name().as_slice() != b"empty?" || def.parameters().is_some() {
        return None;
    }
    Some(def.receiver())
}

/// `block.arguments.empty?`: numbered and `it` parameters are not arguments.
fn block_parameters_empty(block: &ruby_ast::node::BlockNode<'_>) -> bool {
    match block.parameters() {
        None => true,
        Some(parameters) => match parameters.as_block_parameters_node() {
            Some(parameters) => parameters.parameters().is_none() && parameters.locals().is_empty(),
            None => true,
        },
    }
}
