//! `Minitest/RefuteRespondTo`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_respond_to.rb` (via `MinitestCopRule.define_rule`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// Enforces the test to use `refute_respond_to(object, :do_something)` over `refute(object.respond_to?(:do_something))`.
#[derive(Debug, Clone)]
pub struct RefuteRespondTo;

impl Rule for RefuteRespondTo {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteRespondTo",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `refute_respond_to(object, :do_something)` over `refute(object.respond_to?(:do_something))`.",
        explanation: "Enforces the test to use `refute_respond_to(object, :do_something)` over `refute(object.respond_to?(:do_something))`.",
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
        // `RESTRICT_ON_SEND` / `on_send`: a plain `send`, not `csend`.
        if call.is_safe_navigation() || call.name().as_slice() != b"refute" {
            return;
        }
        let arguments = argument_list(&call);
        let Some(first) = arguments.first() else { return };
        // `node.arguments.first&.call_type?`: a call with a block is a `block` node.
        let Some(target) = first.as_call_node() else { return };
        if target.block().is_some_and(|block| block.as_block_node().is_some()) {
            return;
        }
        if argument_list(&target).is_empty() {
            return;
        }
        let name = target.name();
        let name = name.as_slice();
        if name != b"respond_to?" {
            return;
        }

        let new_arguments = new_arguments(&target, ctx).join(", ");
        // `message_argument = arguments.last if arguments.first != arguments.last`
        let last = &arguments[arguments.len() - 1];
        let mut offense_arguments = new_arguments.clone();
        if ctx.text(first.span()) != ctx.text(last.span()) {
            offense_arguments.push_str(", ");
            offense_arguments.push_str(&String::from_utf8_lossy(ctx.text(last.span())));
        }
        let message = format!("Prefer using `refute_respond_to({offense_arguments})`.");

        let Some(selector) = call.message_loc() else { return };
        let edits = vec![
            Edit::replace(selector.span(), b"refute_respond_to".to_vec()),
            Edit::replace(first.span(), new_arguments.into_bytes()),
        ];
        ctx.report_with_fix(
            &Self::META,
            call_span_excluding_block(&call),
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

fn new_arguments(target: &CallNode<'_>, ctx: &Context<'_>) -> Vec<String> {
    let receiver = match target.receiver() {
        Some(receiver) => String::from_utf8_lossy(ctx.text(receiver.span())).into_owned(),
        None => "self".to_owned(),
    };
    let method_argument = argument_list(target)
        .first()
        .map(|argument| String::from_utf8_lossy(ctx.text(argument.span())).into_owned());
    let new_arguments: Vec<String> = std::iter::once(receiver).chain(method_argument).collect();

    new_arguments
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
