//! Helpers shared by the Rails cops in this crate.

use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _};
use ruby_source::Span;

/// A call's arguments the way `parser` sees them: the argument list followed
/// by a `&block` argument, which is one more `send` child there.
pub fn parser_args<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut out: Vec<Node<'pr>> = call
        .arguments()
        .map(|arguments| arguments.arguments().iter().collect())
        .unwrap_or_default();
    if let Some(block) = call.block() {
        if block.as_block_argument_node().is_some() {
            out.push(block);
        }
    }
    out
}

/// `(hash (pair (sym :id) $_))`: a hash literal (braced or bare keywords)
/// whose only element is `id: value`; yields the value.
pub fn sole_id_pair_value<'pr>(hash: &Node<'pr>) -> Option<Node<'pr>> {
    let elements = if let Some(h) = hash.as_hash_node() {
        h.elements()
    } else {
        hash.as_keyword_hash_node()?.elements()
    };
    let mut it = elements.iter();
    let (Some(first), None) = (it.next(), it.next()) else { return None };
    let assoc = first.as_assoc_node()?;
    let key = assoc.key();
    let symbol = key.as_symbol_node()?;
    if symbol.unescaped() == b"id" {
        Some(assoc.value())
    } else {
        None
    }
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
