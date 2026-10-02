//! Helpers shared by this crate's cops.

use linter::{OptionValue, RuleOptions};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _};
use ruby_source::Span;

/// `TargetRailsVersion::DEFAULT_RAILS_VERSION`, used when the configuration
/// states none.
pub const DEFAULT_RAILS_VERSION: f64 = 5.0;

/// `Config#target_rails_version`: `AllCops/TargetRailsVersion` when set.
#[must_use]
pub fn target_rails_version(options: &RuleOptions) -> f64 {
    match options.peer("AllCops", "TargetRailsVersion") {
        Some(OptionValue::Str(text)) => text.trim().parse().unwrap_or(DEFAULT_RAILS_VERSION),
        Some(value) => value.as_float().unwrap_or(DEFAULT_RAILS_VERSION),
        None => DEFAULT_RAILS_VERSION,
    }
}

/// A call's arguments as rubocop-ast's `send` node lists them: the
/// positional arguments followed by a `&block` argument, which whitequark
/// treats as one more argument.
#[must_use]
pub fn send_arguments<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut arguments: Vec<Node<'pr>> =
        call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
    arguments.extend(call.block().filter(|b| b.as_block_argument_node().is_some()));
    arguments
}

/// The span of the `send` node for `call`: its own source without an
/// attached literal block, but including a `&block` argument.
#[must_use]
pub fn send_span(call: &CallNode<'_>) -> Span {
    match call.block() {
        Some(block) if call.closing_loc().is_none() && block.as_block_argument_node().is_some() => {
            Span::new(call.as_node().span().start, block.span().end)
        }
        _ => call_span_excluding_block(call),
    }
}

/// Whether `call` is the associated call of a `:block` node (not a
/// numbered-parameter or `it` block, which parser models as other types);
/// returns the block's span.
#[must_use]
pub fn plain_block_span(call: &CallNode<'_>) -> Option<Span> {
    let block = call.block()?;
    let block_node = block.as_block_node()?;
    let numbered = block_node.parameters().is_some_and(|p| {
        p.as_numbered_parameters_node().is_some() || p.as_it_parameters_node().is_some()
    });
    (!numbered).then(|| block.span())
}
