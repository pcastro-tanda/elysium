//! `Minitest/UnspecifiedException`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/unspecified_exception.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeKind};

const MSG: &str = "Specify the exception being captured.";

/// This cop checks for a specified error in `assert_raises`.
#[derive(Debug, Clone)]
pub struct UnspecifiedException;

impl Rule for UnspecifiedException {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/UnspecifiedException",
        department: Department::Minitest,
        summary: "This cop checks for a specified error in `assert_raises`.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
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
        // `on_block` only: numbered-parameter and `it` blocks are other node types.
        let Some(block) = call.block().and_then(|block| block.as_block_node()) else { return };
        if block.parameters().is_some_and(|parameters| {
            parameters.as_numbered_parameters_node().is_some()
                || parameters.as_it_parameters_node().is_some()
        }) {
            return;
        }
        if call.name().as_slice() != b"assert_raises" {
            return;
        }
        if unspecified_exception(&call, ctx) {
            ctx.report(&Self::META, call_span_excluding_block(&call), MSG);
        }
    }
}

/// `args.empty? || (args.size == 1 && args[0].str_type?)`.
fn unspecified_exception(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    let mut args: Vec<Node<'_>> =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block().filter(|block| block.as_block_argument_node().is_some()) {
        args.push(block);
    }
    match args.as_slice() {
        [] => true,
        [only] => str_type(only, ctx),
        _ => false,
    }
}

/// whitequark's `str` (not `dstr`): a plain string literal that does not span
/// several lines (the parser splits those into a `dstr`).
fn str_type(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let Some(string) = node.as_string_node() else { return false };
    let content = ctx.text(string.content_loc().span());
    match content.iter().position(|&b| b == b'\n') {
        Some(index) => index + 1 == content.len(),
        None => true,
    }
}
