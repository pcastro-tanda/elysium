//! `Minitest/RefuteInstanceOf`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_instance_of.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const ASSERTION_TYPE: &str = "refute";

/// This cop enforces the test to use `refute_instance_of(Class, object)` over `refute(object.instance_of?(Class))`.
#[derive(Debug, Clone)]
pub struct RefuteInstanceOf;

impl Rule for RefuteInstanceOf {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteInstanceOf",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `refute_instance_of(Class, object)` over `refute(object.instance_of?(Class))`.",
        explanation: "",
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
        let Some((receiver, constant, message_index)) = instance_of_assertion(&call, &arguments)
        else {
            return;
        };
        // `(send nil? ...)` capturing a `nil` receiver would make
        // `build_required_arguments` call `source` on `nil`.
        let Some(receiver) = receiver else { return };

        let receiver_source = String::from_utf8_lossy(ctx.text(receiver.span())).into_owned();
        let constant_source = String::from_utf8_lossy(ctx.text(constant.span())).into_owned();
        let required_arguments = format!("{constant_source}, {receiver_source}");
        let full_arguments = match arguments.get(message_index) {
            Some(message) => {
                format!(
                    "{required_arguments}, {}",
                    String::from_utf8_lossy(ctx.text(message.span()))
                )
            }
            None => required_arguments.clone(),
        };
        let prefer = format!("{ASSERTION_TYPE}_instance_of({full_arguments})");

        let Some(selector) = call.message_loc() else { return };
        let range = if call.name().as_slice() == ASSERTION_TYPE.as_bytes() {
            arguments[0].span()
        } else {
            Span::new(arguments[0].span().start, arguments[1].span().end)
        };
        ctx.report_with_fix(
            &Self::META,
            call_span_excluding_block(&call),
            format!("Prefer using `{prefer}`."),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::replace(
                        selector.span(),
                        format!("{ASSERTION_TYPE}_instance_of").into_bytes(),
                    ),
                    Edit::replace(range, required_arguments.into_bytes()),
                ],
            },
        );
    }
}

/// The `instance_of_assertion?` node pattern:
///
/// ```text
/// {
///   (send nil? :ASSERT (send $_ :instance_of? $const) $_?)
///   (send nil? :ASSERT_equal $const (send $_ :class) $_?)
/// }
/// ```
///
/// Returns the captured receiver (`None` when the inner call has none), the
/// constant, and the index where the optional message argument sits.
#[allow(clippy::type_complexity)]
fn instance_of_assertion<'pr>(
    call: &CallNode<'pr>,
    arguments: &[Node<'pr>],
) -> Option<(Option<Node<'pr>>, Node<'pr>, usize)> {
    let name = call.name();
    let name = name.as_slice();
    if name == ASSERTION_TYPE.as_bytes() {
        if arguments.is_empty() || arguments.len() > 2 {
            return None;
        }
        let inner = plain_send(&arguments[0], b"instance_of?")?;
        let inner_arguments = argument_list(&inner);
        let [constant] = inner_arguments.as_slice() else { return None };
        if !is_const(constant) {
            return None;
        }
        Some((inner.receiver(), *constant, 1))
    } else if name == format!("{ASSERTION_TYPE}_equal").as_bytes() {
        if arguments.len() < 2 || arguments.len() > 3 {
            return None;
        }
        if !is_const(&arguments[0]) {
            return None;
        }
        let inner = plain_send(&arguments[1], b"class")?;
        if !argument_list(&inner).is_empty() {
            return None;
        }
        Some((inner.receiver(), arguments[0], 2))
    } else {
        None
    }
}

/// `(send _ :name ...)`: a non-safe-navigation call with no block literal.
fn plain_send<'pr>(node: &Node<'pr>, name: &[u8]) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation()
        || call.name().as_slice() != name
        || call.block().is_some_and(|block| block.as_block_node().is_some())
    {
        return None;
    }
    Some(call)
}

fn is_const(node: &Node<'_>) -> bool {
    node.as_constant_read_node().is_some() || node.as_constant_path_node().is_some()
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
