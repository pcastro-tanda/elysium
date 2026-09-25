//! Node identity: the one thing Prism's Rust API cannot express directly.

use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Identity of a node in one parsed file: its kind plus its byte range.
///
/// Prism returns a fresh `Node` value each time an accessor is called, so
/// pointer identity (RuboCop's `equal?`) is unavailable; no two distinct
/// nodes in one tree share both a kind and a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId {
    /// The node's kind.
    pub kind: NodeKind,
    /// The node's byte range.
    pub span: Span,
}

/// Identity of `node`.
#[must_use]
pub fn node_id(node: &Node<'_>) -> NodeId {
    NodeId { kind: node.kind(), span: node.span() }
}

/// RuboCop's `equal?` on nodes.
#[must_use]
pub fn same(a: &Node<'_>, b: &Node<'_>) -> bool {
    node_id(a) == node_id(b)
}

/// True when `a` is present and identical to `b`.
#[must_use]
pub(crate) fn same_opt(a: Option<&Node<'_>>, b: &Node<'_>) -> bool {
    a.is_some_and(|a| same(a, b))
}
