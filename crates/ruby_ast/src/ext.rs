//! Cross-rule node predicates that do not need source text, promoted out of
//! the several rule files that each grew their own copy. See
//! `rubocop-ast`'s `Node`/`RuboCop::AST::MethodDispatchNode` mixins for the
//! upstream originals.

use crate::node::CallNode;
use crate::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `rubocop-ast`'s `Node#heredoc?`: true for a heredoc string/xstring
/// literal -- one of the four string-literal kinds whose `opening_loc`
/// starts with `<<`.
#[must_use]
pub fn is_heredoc(node: &Node<'_>) -> bool {
    let opening = match node {
        Node::StringNode { .. } => node.as_string_node().and_then(|n| n.opening_loc()),
        Node::InterpolatedStringNode { .. } => {
            node.as_interpolated_string_node().and_then(|n| n.opening_loc())
        }
        Node::XStringNode { .. } => node.as_x_string_node().map(|n| n.opening_loc()),
        Node::InterpolatedXStringNode { .. } => {
            node.as_interpolated_x_string_node().map(|n| n.opening_loc())
        }
        _ => None,
    };
    opening.is_some_and(|loc| loc.as_slice().starts_with(b"<<"))
}

/// `(const {nil? cbase} :Proc)`: a bare or top-level-qualified `Proc`
/// constant, used as the receiver of `.new` by [`is_lambda_or_proc`].
fn is_proc_const(node: &Node<'_>) -> bool {
    match node {
        Node::ConstantReadNode { .. } => {
            node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"Proc")
        }
        Node::ConstantPathNode { .. } => node.as_constant_path_node().is_some_and(|path| {
            path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == b"Proc")
        }),
        _ => false,
    }
}

/// `rubocop-ast`'s `lambda_or_proc?`, restricted to the `CallNode` shapes
/// that can ever reach it: `lambda { }`, `proc { }`, `Proc.new { }`, or
/// `::Proc.new { }`. A bare method named `proc`/`lambda` on an explicit
/// receiver (e.g. `Foo.proc { }`) does not match, per RuboCop's
/// `(send nil? :proc)`/`(send nil? :lambda)` alternatives.
#[must_use]
pub fn is_lambda_or_proc(call: &CallNode<'_>) -> bool {
    let name = call.name();
    let name = name.as_slice();
    if call.receiver().is_none() && (name == b"lambda" || name == b"proc") {
        return true;
    }
    name == b"new" && call.receiver().is_some_and(|r| is_proc_const(&r))
}

/// RuboCop's `bare_access_modifier?`/`access_modifier?`: a receiver-less,
/// argument-less call to `private`/`protected`/`public`/`module_function`.
#[must_use]
pub fn is_bare_access_modifier(call: &CallNode<'_>) -> bool {
    call.receiver().is_none()
        && call.arguments().is_none()
        && matches!(
            call.name().as_slice(),
            b"private" | b"protected" | b"public" | b"module_function"
        )
}

/// `rubocop-ast`'s `(const {nil? cbase} :Name)` shape: a bare `Name`
/// constant reference, or one qualified only by a leading `::` (`cbase`)
/// with no intervening namespace. This checks the shape only; callers that
/// need to match a specific constant name compare it themselves once this
/// passes.
#[must_use]
pub fn is_bare_or_toplevel_const(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::ConstantReadNode => true,
        NodeKind::ConstantPathNode => {
            node.as_constant_path_node().is_some_and(|path| path.parent().is_none())
        }
        _ => false,
    }
}

/// The end of `call`'s own source, excluding any attached block: RuboCop's
/// `send_node.source` for a block's associated call, ported since a
/// `CallNode`'s span always extends through its own attached block.
fn call_end_excluding_block(call: &CallNode<'_>) -> u32 {
    if let Some(closing) = call.closing_loc() {
        return closing.span().end;
    }
    if let Some(args) = call.arguments() {
        if let Some(last) = args.arguments().last() {
            return last.span().end;
        }
    }
    call.message_loc().map_or_else(|| call.as_node().span().start, |loc| loc.span().end)
}

/// `call`'s own span, excluding any attached block.
#[must_use]
pub fn call_span_excluding_block(call: &CallNode<'_>) -> Span {
    Span::new(call.as_node().span().start, call_end_excluding_block(call))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{walk, Parsed, Visitor};
    use ruby_source::SourceFile;

    fn parse(text: &str) -> SourceFile {
        SourceFile::new("test.rb", text.as_bytes().to_vec())
    }

    /// Depth-first search that evaluates `check` against every node, short-circuiting on the
    /// first `Some`. `Node`/`CallNode` are not `Copy`, so the result is computed inline (a
    /// `bool`/`Span`/etc.) rather than smuggling a borrowed node out of the traversal.
    fn first_where<'pr, T>(root: &Node<'pr>, mut check: impl FnMut(&Node<'pr>) -> Option<T>) -> T {
        struct Finder<T, F> {
            check: F,
            found: Option<T>,
        }
        impl<'pr, T, F: FnMut(&Node<'pr>) -> Option<T>> Visitor<'pr> for Finder<T, F> {
            fn enter(&mut self, node: &Node<'pr>) {
                if self.found.is_none() {
                    self.found = (self.check)(node);
                }
            }
        }
        let mut finder = Finder { check: &mut check, found: None };
        walk(root, &mut finder);
        finder.found.expect("no matching node found")
    }

    #[test]
    fn is_heredoc_true_for_heredoc_string_false_for_plain_string() {
        let source = parse("x = <<~SQL\n  SELECT 1\nSQL\n");
        let parsed = Parsed::parse(&source);
        let heredoc = first_where(&parsed.root(), |n| {
            matches!(n, Node::StringNode { .. }).then(|| is_heredoc(n))
        });
        assert!(heredoc);

        let source = parse("x = \"plain\"\n");
        let parsed = Parsed::parse(&source);
        let plain = first_where(&parsed.root(), |n| {
            matches!(n, Node::StringNode { .. }).then(|| is_heredoc(n))
        });
        assert!(!plain);
    }

    #[test]
    fn is_lambda_or_proc_matches_lambda_proc_and_proc_new_not_receiver_proc() {
        for good in ["lambda { }", "proc { }", "Proc.new { }", "::Proc.new { }"] {
            let source = parse(good);
            let parsed = Parsed::parse(&source);
            let matched =
                first_where(&parsed.root(), |n| n.as_call_node().map(|c| is_lambda_or_proc(&c)));
            assert!(matched, "expected {good:?} to match");
        }

        let source = parse("Foo.proc { }\n");
        let parsed = Parsed::parse(&source);
        let matched =
            first_where(&parsed.root(), |n| n.as_call_node().map(|c| is_lambda_or_proc(&c)));
        assert!(!matched);
    }

    #[test]
    fn is_bare_access_modifier_rejects_args_and_receiver() {
        let source = parse("private\n");
        let parsed = Parsed::parse(&source);
        assert!(first_where(&parsed.root(), |n| n
            .as_call_node()
            .map(|c| is_bare_access_modifier(&c))));

        let source = parse("private :foo\n");
        let parsed = Parsed::parse(&source);
        assert!(!first_where(&parsed.root(), |n| n
            .as_call_node()
            .map(|c| is_bare_access_modifier(&c))));

        let source = parse("Foo.private\n");
        let parsed = Parsed::parse(&source);
        assert!(!first_where(&parsed.root(), |n| n
            .as_call_node()
            .map(|c| is_bare_access_modifier(&c))));
    }

    #[test]
    fn is_bare_or_toplevel_const_rejects_qualified_names() {
        let source = parse("Foo\n");
        let parsed = Parsed::parse(&source);
        assert!(first_where(&parsed.root(), |n| matches!(n, Node::ConstantReadNode { .. })
            .then(|| is_bare_or_toplevel_const(n))));

        let source = parse("::Foo\n");
        let parsed = Parsed::parse(&source);
        assert!(first_where(&parsed.root(), |n| matches!(n, Node::ConstantPathNode { .. })
            .then(|| is_bare_or_toplevel_const(n))));

        let source = parse("Foo::Bar\n");
        let parsed = Parsed::parse(&source);
        assert!(!first_where(&parsed.root(), |n| matches!(n, Node::ConstantPathNode { .. })
            .then(|| is_bare_or_toplevel_const(n))));
    }

    #[test]
    fn call_span_excluding_block_stops_before_braces() {
        let source = parse("foo(1, 2) { }\n");
        let parsed = Parsed::parse(&source);
        let span = first_where(&parsed.root(), |n| {
            n.as_call_node().map(|c| call_span_excluding_block(&c))
        });
        assert_eq!(&source.bytes()[span.start as usize..span.end as usize], b"foo(1, 2)");

        let source = parse("foo { }\n");
        let parsed = Parsed::parse(&source);
        let span = first_where(&parsed.root(), |n| {
            n.as_call_node().map(|c| call_span_excluding_block(&c))
        });
        assert_eq!(&source.bytes()[span.start as usize..span.end as usize], b"foo");
    }
}
