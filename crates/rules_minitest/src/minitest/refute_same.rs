//! `Minitest/RefuteSame`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_same.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Enforces the use of `refute_same(expected, actual)` over `refute(expected.equal?(actual))`.
#[derive(Debug, Clone)]
pub struct RefuteSame;

impl Rule for RefuteSame {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteSame",
        department: Department::Minitest,
        summary: "Enforces the use of `refute_same(expected, actual)` over `refute(expected.equal?(actual))`.",
        explanation: "Enforces the use of `refute_same(expected, actual)` over `refute(expected.equal?(actual))`.\n\nUse `refute_same` only when there is a need to compare by identity. Otherwise, use `refute_equal`.\n\n```ruby\n# bad\nrefute(expected.equal?(actual))\nrefute_equal(expected.object_id, actual.object_id)\n\n# good\nrefute_same(expected, actual)\n```",
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
        let Some(selector) = call.message_loc() else { return };
        let source = |node: &Node<'_>| String::from_utf8_lossy(ctx.text(node.span())).into_owned();
        let join = |parts: &[&Node<'_>]| {
            parts.iter().map(|part| source(part)).collect::<Vec<_>>().join(", ")
        };

        match call.name().as_slice() {
            b"refute" => {
                // `(send nil? :refute $(send $_ :equal? $_) $_?)`
                if arguments.len() > 2 {
                    return;
                }
                let Some(equal) = arguments.first().and_then(Node::as_call_node) else { return };
                if equal.name().as_slice() != b"equal?" || !is_plain_send(&equal) {
                    return;
                }
                let equal_arguments = argument_list(&equal);
                let ([actual], Some(expected)) = (equal_arguments.as_slice(), equal.receiver())
                else {
                    return;
                };
                let mut shown = vec![&expected, actual];
                shown.extend(arguments.get(1));
                let message = format!("Prefer using `refute_same({})`.", join(&shown));
                let new_equal = join(&[&expected, actual]);
                ctx.report_with_fix(
                    &Self::META,
                    call_span_excluding_block(&call),
                    message,
                    Fix {
                        applicability: Applicability::Safe,
                        edits: vec![
                            Edit::replace(selector.span(), b"refute_same".to_vec()),
                            Edit::replace(arguments[0].span(), new_equal.into_bytes()),
                        ],
                    },
                );
            }
            b"refute_equal" => {
                // `(send nil? :refute_equal (send $_ :object_id) (send $_ :object_id) $_?)`
                if arguments.len() < 2 || arguments.len() > 3 {
                    return;
                }
                let (Some(first), Some(second)) =
                    (object_id_call(&arguments[0]), object_id_call(&arguments[1]))
                else {
                    return;
                };
                let (Some(expected), Some(actual)) = (first.receiver(), second.receiver()) else {
                    return;
                };
                let mut shown = vec![&expected, &actual];
                shown.extend(arguments.get(2));
                let message = format!("Prefer using `refute_same({})`.", join(&shown));
                let mut edits = vec![Edit::replace(selector.span(), b"refute_same".to_vec())];
                for object_id in [&first, &second] {
                    // `remove_method_call`: from the dot through the selector.
                    if let (Some(dot), Some(name)) =
                        (object_id.call_operator_loc(), object_id.message_loc())
                    {
                        edits.push(Edit::delete(Span::new(dot.span().start, name.span().end)));
                    }
                }
                ctx.report_with_fix(
                    &Self::META,
                    call_span_excluding_block(&call),
                    message,
                    Fix { applicability: Applicability::Safe, edits },
                );
            }
            _ => {}
        }
    }
}

/// `(send $_ :object_id)`
fn object_id_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    (call.name().as_slice() == b"object_id"
        && is_plain_send(&call)
        && call.arguments().is_none()
        && call.block().is_none())
    .then_some(call)
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
