//! `Minitest/RedundantMessageArgument`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/redundant_message_argument.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Remove the redundant message argument.";

/// `RESTRICT_ON_SEND` paired with the `_` count that precedes the `$nil`
/// message argument in `redundant_message_argument`.
const ASSERTIONS: &[(&[u8], usize)] = &[
    (b"assert", 1),
    (b"assert_empty", 1),
    (b"assert_equal", 2),
    (b"assert_in_delta", 3),
    (b"assert_in_epsilon", 3),
    (b"assert_includes", 2),
    (b"assert_instance_of", 2),
    (b"assert_kind_of", 2),
    (b"assert_match", 2),
    (b"assert_nil", 1),
    (b"assert_operator", 3),
    (b"assert_path_exists", 1),
    (b"assert_predicate", 2),
    (b"assert_respond_to", 2),
    (b"assert_same", 2),
    (b"assert_throws", 1),
    (b"flunk", 0),
    (b"refute", 1),
    (b"refute_empty", 1),
    (b"refute_equal", 2),
    (b"refute_in_delta", 3),
    (b"refute_in_epsilon", 3),
    (b"refute_includes", 2),
    (b"refute_instance_of", 2),
    (b"refute_kind_of", 2),
    (b"refute_match", 2),
    (b"refute_nil", 1),
    (b"refute_operator", 3),
    (b"refute_path_exists", 1),
    (b"refute_predicate", 2),
    (b"refute_respond_to", 2),
    (b"refute_same", 2),
];

/// Detects redundant message argument in assertion methods.
/// The message argument `nil` is redundant because it is the default value.
#[derive(Debug, Clone)]
pub struct RedundantMessageArgument;

impl Rule for RedundantMessageArgument {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RedundantMessageArgument",
        department: Department::Minitest,
        summary: "Detects redundant message argument in assertion methods.",
        explanation: "Detects redundant message argument in assertion methods. The message \
                      argument `nil` is redundant because it is the default value.\n\n\
                      ```ruby\n# bad\nassert_equal(expected, actual, nil)\n\n# good\n\
                      assert_equal(expected, actual)\nassert_equal(expected, actual, 'message')\n```",
        enabled_by_default: false,
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
        // `on_send` does not fire for `csend`.
        if call.receiver().is_some() || call.is_safe_navigation() {
            return;
        }
        let arguments = argument_list(&call);
        let Some(redundant) = redundant_message_argument(&call, &arguments) else { return };

        let range = if arguments.len() == 1 {
            redundant.span()
        } else {
            // `node.arguments.index(redundant_message_argument)` compares
            // nodes structurally: the first `nil` argument, which may not be
            // the matched one. Index 0 wraps to `arguments[-1]`.
            let index = arguments.iter().position(|arg| arg.as_nil_node().is_some()).unwrap_or(0);
            let previous = match index.checked_sub(1) {
                Some(previous) => &arguments[previous],
                None => &arguments[arguments.len() - 1],
            };
            Span::new(previous.span().end, redundant.span().end)
        };
        ctx.report_with_fix(
            &Self::META,
            redundant.span(),
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(range)] },
        );
    }
}

/// The `redundant_message_argument` node pattern: `(send nil? :name _* $nil)`
/// with the exact arity of the assertion.
fn redundant_message_argument<'pr>(
    call: &CallNode<'pr>,
    arguments: &[Node<'pr>],
) -> Option<Node<'pr>> {
    let name = call.name();
    let (_, before) = ASSERTIONS.iter().find(|(assertion, _)| *assertion == name.as_slice())?;
    if arguments.len() != before + 1 {
        return None;
    }
    let message = arguments[*before];
    message.as_nil_node()?;
    Some(message)
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
