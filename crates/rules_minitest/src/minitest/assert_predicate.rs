//! `Minitest/AssertPredicate`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_predicate.rb` (with its
//! `PredicateAssertionHandleable` mixin).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const ASSERTION_TYPE: &str = "assert";

/// Enforces the test to use `assert_predicate` instead of using
/// `assert(obj.a_predicate_method?)`.
#[derive(Debug, Clone)]
pub struct AssertPredicate;

impl Rule for AssertPredicate {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertPredicate",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `assert_predicate` instead of using `assert(obj.a_predicate_method?)`.",
        explanation: "Enforces the test to use `assert_predicate` instead of using \
                      `assert(obj.a_predicate_method?)`.\n\n```ruby\n# bad\n\
                      assert(obj.one?)\nassert(obj.one?, 'message')\n\n# good\n\
                      assert_predicate(obj, :one?)\n\
                      assert_predicate(obj, :one?, 'message')\n```",
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
        if call.name().as_slice() != ASSERTION_TYPE.as_bytes() {
            return;
        }
        let arguments = argument_list(&call);
        let Some(first) = arguments.first() else { return };
        // `return if node.first_argument&.any_block_type?`, then `predicate_method?`.
        // `predicate_method?` is also defined on `def`/`defs` nodes, whose
        // `arguments` are their parameters and `receiver` the `defs` target.
        let (name, receiver, has_arguments) = if let Some(predicate) = first.as_call_node() {
            if predicate.block().is_some_and(|block| block.as_block_argument_node().is_none()) {
                return;
            }
            (
                predicate.name().as_slice().to_vec(),
                predicate.receiver(),
                !argument_list(&predicate).is_empty(),
            )
        } else if let Some(def) = first.as_def_node() {
            (def.name().as_slice().to_vec(), def.receiver(), def.parameters().is_some())
        } else {
            return;
        };
        if !name.ends_with(b"?") || has_arguments {
            return;
        }

        let receiver = match receiver {
            Some(receiver) => String::from_utf8_lossy(ctx.text(receiver.span())).into_owned(),
            None => "self".to_owned(),
        };
        let new_arguments = format!("{receiver}, :{}", String::from_utf8_lossy(&name));
        let message_argument = if arguments.len() > 1 { arguments.last() } else { None };
        let full = match message_argument {
            Some(message_argument) => format!(
                "{new_arguments}, {}",
                String::from_utf8_lossy(ctx.text(message_argument.span()))
            ),
            None => new_arguments.clone(),
        };
        let message = format!("Prefer using `{ASSERTION_TYPE}_predicate({full})`.");

        let Some(selector) = call.message_loc() else { return };
        let edits = vec![
            Edit::replace(selector.span(), format!("{ASSERTION_TYPE}_predicate").into_bytes()),
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
