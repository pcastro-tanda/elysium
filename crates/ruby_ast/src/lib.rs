//! Thin facade over the `ruby-prism` parser.
//!
//! Every other crate reaches Prism only through this module. Rules import
//! node types from [`node`], parse through [`Parsed`], and traverse with
//! [`walk`]. If a future Prism release changes its Rust API, this crate is
//! the only place that has to move.
//!
//! The facade deliberately does not re-wrap the 151 node structs: Prism's
//! generated accessors are already zero-cost and well named. What it adds is
//! a stable [`NodeKind`] discriminant (generated from Prism's own node schema
//! at build time), byte-offset [`Span`]s, and an enter/leave [`Visitor`] that
//! drives one traversal for every consumer.

use ruby_source::{SourceFile, Span};

mod generated {
    #![allow(clippy::wildcard_imports, clippy::too_many_lines, clippy::match_same_arms)]
    use ruby_prism::{Node, Visit};
    include!(concat!(env!("OUT_DIR"), "/node_kind.rs"));
}

pub use generated::{kind_of, visit_children, NodeKind};

/// Re-export of every Prism node type, list type, and helper.
///
/// Rules and the semantic layer use these types directly; only `parse` from
/// this module should be avoided in favour of [`Parsed::parse`].
pub mod node {
    pub use ruby_prism::*;
}

pub use node::{Comment, CommentType, Location, MagicComment, Node, NodeList};

/// Cross-rule node predicates needing no source text. See [`ext`].
pub mod ext;

/// Ruby syntax version to parse as. Prism only models 3.3 and newer; a
/// `TargetRubyVersion` older than that parses as 3.3.
pub use ruby_prism::SyntaxVersion as RubyVersion;

/// Parser options. The default matches how RuboCop drives Prism through
/// `Prism::Translation::Parser`: top-level `yield`/`return` are allowed
/// (`partial_script`) because templates and DSL files rely on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseOptions {
    /// Syntax version.
    pub version: RubyVersion,
    /// Allow top-level `yield`, `return`, `redo`, `retry`.
    pub partial_script: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self { version: RubyVersion::Latest, partial_script: true }
    }
}

/// A parsed file: the Prism result plus the source it borrows from.
pub struct Parsed<'src> {
    source: &'src SourceFile,
    result: ruby_prism::ParseResult<'src>,
}

impl<'src> Parsed<'src> {
    /// Parses `source` with the default [`ParseOptions`].
    pub fn parse(source: &'src SourceFile) -> Self {
        Self::parse_with(source, ParseOptions::default())
    }

    /// Parses `source` with Prism. Never fails: syntax errors are reported
    /// through [`Parsed::errors`] and the tree is recovered as far as Prism
    /// can manage.
    pub fn parse_with(source: &'src SourceFile, options: ParseOptions) -> Self {
        let filepath = source.path().as_os_str().as_encoded_bytes();
        let prism_options = ruby_prism::ParseOptions {
            filepath: Some(filepath),
            version: options.version,
            partial_script: options.partial_script,
            ..ruby_prism::ParseOptions::default()
        };
        Self { source, result: ruby_prism::parse_with_options(source.bytes(), &prism_options) }
    }

    /// The source this tree was parsed from.
    pub fn source(&self) -> &'src SourceFile {
        self.source
    }

    /// The root `ProgramNode`, as a generic node.
    pub fn root(&self) -> Node<'_> {
        self.result.node()
    }

    /// Syntax errors reported by Prism.
    pub fn errors(&self) -> impl Iterator<Item = SyntaxError> + '_ {
        self.result
            .errors()
            .map(|d| SyntaxError { message: d.message().to_owned(), span: d.location().span() })
    }

    /// Parse-time warnings reported by Prism.
    pub fn warnings(&self) -> impl Iterator<Item = SyntaxError> + '_ {
        self.result
            .warnings()
            .map(|d| SyntaxError { message: d.message().to_owned(), span: d.location().span() })
    }

    /// True when Prism reported at least one error.
    pub fn has_errors(&self) -> bool {
        self.result.errors().next().is_some()
    }

    /// All comments in source order.
    pub fn comments(&self) -> impl Iterator<Item = Comment<'_>> + '_ {
        self.result.comments()
    }

    /// Magic comments (`# frozen_string_literal: true` and friends).
    pub fn magic_comments(&self) -> impl Iterator<Item = MagicComment<'_>> + '_ {
        self.result.magic_comments()
    }

    /// True when a `frozen_string_literal: true` magic comment is present.
    pub fn frozen_string_literals(&self) -> bool {
        self.result.frozen_string_literals()
    }

    /// Location of the `__END__` data section, if any.
    pub fn data_span(&self) -> Option<Span> {
        self.result.data_loc().map(|l| l.span())
    }
}

/// A syntax error or warning from the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    /// Prism's message text.
    pub message: String,
    /// Byte range the diagnostic points at.
    pub span: Span,
}

/// Byte-offset conversion for Prism locations.
pub trait LocationExt {
    /// The location as a [`Span`].
    fn span(&self) -> Span;
}

impl LocationExt for Location<'_> {
    fn span(&self) -> Span {
        Span::from_usize(self.start_offset(), self.end_offset())
    }
}

/// Convenience accessors on generic nodes.
pub trait NodeExt {
    /// The node's kind discriminant.
    fn kind(&self) -> NodeKind;
    /// The node's full byte range.
    fn span(&self) -> Span;
}

impl NodeExt for Node<'_> {
    fn kind(&self) -> NodeKind {
        kind_of(self)
    }

    fn span(&self) -> Span {
        self.location().span()
    }
}

/// Enter/leave visitor over a Prism tree. Both hooks have empty defaults.
pub trait Visitor<'pr> {
    /// Called before the node's children are visited.
    fn enter(&mut self, node: &Node<'pr>) {
        let _ = node;
    }

    /// Called after the node's children have been visited.
    fn leave(&mut self, node: &Node<'pr>) {
        let _ = node;
    }
}

struct Adapter<'a, V> {
    inner: &'a mut V,
}

impl<'pr, V: Visitor<'pr>> ruby_prism::Visit<'pr> for Adapter<'_, V> {
    fn visit(&mut self, node: &Node<'pr>) {
        self.inner.enter(node);
        visit_children(self, node);
        self.inner.leave(node);
    }
}

/// Walks `root` depth-first, calling `enter` before and `leave` after each
/// node's children.
pub fn walk<'pr, V: Visitor<'pr>>(root: &Node<'pr>, visitor: &mut V) {
    use ruby_prism::Visit as _;
    Adapter { inner: visitor }.visit(root);
}

/// Calls `f` for each direct child of `node`, in Prism field order --
/// rubocop-ast's `Node#each_child_node` without the visitor boilerplate.
pub fn for_each_child<'pr>(node: &Node<'pr>, f: impl FnMut(&Node<'pr>)) {
    struct Shim<F>(F);
    impl<'pr, F: FnMut(&Node<'pr>)> ruby_prism::Visit<'pr> for Shim<F> {
        fn visit(&mut self, node: &Node<'pr>) {
            (self.0)(node);
        }
    }
    visit_children(&mut Shim(f), node);
}

/// Calls `f` for every descendant of `node`, pre-order -- rubocop-ast's
/// `Node#each_descendant`. Scope boundaries are not respected, matching the
/// upstream helper.
pub fn each_descendant<'pr, F: FnMut(&Node<'pr>)>(node: &Node<'pr>, f: &mut F) {
    for_each_child(node, |child| {
        f(child);
        each_descendant(child, f);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(text: &str) -> SourceFile {
        SourceFile::new("test.rb", text.as_bytes().to_vec())
    }
    #[derive(Default)]
    struct Counter {
        entered: Vec<NodeKind>,
        left: Vec<NodeKind>,
    }
    impl<'pr> Visitor<'pr> for Counter {
        fn enter(&mut self, node: &Node<'pr>) {
            self.entered.push(node.kind());
        }
        fn leave(&mut self, node: &Node<'pr>) {
            self.left.push(node.kind());
        }
    }

    #[test]
    fn parses_and_walks_every_node_once() {
        let source = src("def foo(a)\n  a + 1\nend\n");
        let parsed = Parsed::parse(&source);
        assert!(!parsed.has_errors());

        let mut counter = Counter::default();
        walk(&parsed.root(), &mut counter);
        assert_eq!(counter.entered.len(), counter.left.len());
        assert_eq!(counter.entered[0], NodeKind::ProgramNode);
        assert_eq!(*counter.left.last().unwrap(), NodeKind::ProgramNode);
        assert!(counter.entered.contains(&NodeKind::DefNode));
        // Single-kind fields: Prism's default traversal skips `visit` for these.
        assert!(counter.entered.contains(&NodeKind::StatementsNode));
        assert!(counter.entered.contains(&NodeKind::ArgumentsNode));
        assert!(counter.entered.contains(&NodeKind::ParametersNode));
        assert!(counter.entered.contains(&NodeKind::CallNode));
        assert!(counter.entered.contains(&NodeKind::RequiredParameterNode));
        assert!(counter.entered.contains(&NodeKind::LocalVariableReadNode));
        assert!(counter.entered.contains(&NodeKind::IntegerNode));
    }

    #[test]
    fn reports_syntax_errors_with_spans() {
        let source = src("def foo(a)\n  a +\nend\n");
        let parsed = Parsed::parse(&source);
        let errors: Vec<_> = parsed.errors().collect();
        assert!(!errors.is_empty());
        let first = &errors[0];
        assert!(first.span.end as usize <= source.bytes().len());
        assert!(!first.message.is_empty());
    }

    #[test]
    fn top_level_yield_is_allowed_like_rubocop() {
        // Templates (`.builder`, `.jbuilder`) yield at the top level; the
        // parser gem accepts this and so RuboCop never reports it.
        let source = src("xml.feed do\n  xml << yield\nend\n");
        assert!(!Parsed::parse(&source).has_errors());
        let strict = ParseOptions { partial_script: false, ..ParseOptions::default() };
        assert!(Parsed::parse_with(&source, strict).has_errors());
    }

    #[test]
    fn node_kind_names_round_trip() {
        for kind in NodeKind::ALL {
            assert_eq!(NodeKind::from_prism_name(kind.prism_name()), Some(kind));
        }
        assert_eq!(NodeKind::CallNode.prism_name(), "CallNode");
        assert!(NodeKind::IntegerNode.is_leaf());
        assert!(!NodeKind::CallNode.is_leaf());
    }
}
