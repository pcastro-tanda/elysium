//! Helpers shared by the Rails cops in this crate.

use ruby_ast::node::CallNode;
use ruby_ast::Node;

/// A call's arguments the way `parser` sees them: the argument list followed
/// by a `&block` argument, which is one more `send` child there.
pub fn parser_args<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut out: Vec<Node<'pr>> =
        call.arguments().map(|arguments| arguments.arguments().iter().collect()).unwrap_or_default();
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
