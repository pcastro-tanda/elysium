//! `Style/MissingRespondToMissing`, ported from RuboCop's
//! `lib/rubocop/cop/style/missing_respond_to_missing.rb`.
//!
//! # Scope lookup
//!
//! Upstream's `implements_respond_to_missing?` reaches the def's owning
//! scope via `node.parent.parent`: whitequark only wraps a multi-statement
//! body in a `begin` node, so for a lone `method_missing` (no sibling
//! statements) that hop lands *above* the owning class/module, immediately
//! finding nothing -- correctly, since there is no sibling to find. A
//! `private def method_missing; end` needs only that same single hop too,
//! since whitequark's `send` node holds the wrapped `def` as a direct
//! child (no extra wrapper for its argument list).
//!
//! Prism always wraps a body in a `StatementsNode` regardless of statement
//! count (see `Lint::ConstantDefinitionInBlock`'s module doc for the
//! general observation), and *does* insert an extra `ArgumentsNode`
//! between a wrapping `CallNode` (e.g. `private`) and the `def` it wraps.
//! [`MissingRespondToMissing::owning_scope`] reproduces upstream's target
//! in both shapes at once: the nearest enclosing `StatementsNode` among
//! the def's ancestors (its own immediate parent when un-wrapped; two
//! hops further out, past the `CallNode`/`ArgumentsNode`, when wrapped)
//! and *that* node's own parent -- always the actual owning class, module,
//! singleton class, program, or other body-holding construct.
//! [`Context::ancestors`] carries only kind and span, not a node
//! reference, so [`MissingRespondToMissing::node_at`] re-finds that
//! ancestor's actual node by walking down from the parsed tree's root
//! once, matching on its `(kind, span)` pair (unique per node, like
//! `ruby_semantic`'s `NodeId`).
//!
//! # Same-kind pairing
//!
//! Upstream only pairs a `method_missing` with a `respond_to_missing?` of
//! the same `node.type` (`each_descendant(node.type)`: `:def` only matches
//! sibling `:def`s, `:defs` only sibling `:defs`) -- an instance
//! `method_missing` paired with a singleton `respond_to_missing?` (or vice
//! versa) still offends. Prism spells both as the same `DefNode` kind, so
//! this port compares `DefNode::receiver` presence (`Some` means a
//! singleton method) instead of node kind to reproduce the same pairing.
//!
//! # Nested search
//!
//! `each_descendant` (both upstream's and [`ruby_ast::each_descendant`])
//! does not respect scope boundaries, so a `respond_to_missing?` nested
//! anywhere in the owning scope's subtree counts -- including inside a
//! `private def respond_to_missing?; end` (parsed as a `CallNode` wrapping
//! a `DefNode`) and, as a documented blind spot shared with upstream, even
//! inside an unrelated nested class/module within the same enclosing body.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "When using `method_missing`, define `respond_to_missing?`.";

/// Checks for the presence of `method_missing` without also
/// defining `respond_to_missing?`.
///
/// # Examples
///
/// ```ruby
/// # bad
/// def method_missing(name, *args)
///   if @delegate.respond_to?(name)
///     @delegate.send(name, *args)
///   else
///     super
///   end
/// end
///
/// # good
/// def respond_to_missing?(name, include_private)
///   @delegate.respond_to?(name) || super
/// end
///
/// def method_missing(name, *args)
///   if @delegate.respond_to?(name)
///     @delegate.send(name, *args)
///   else
///     super
///   end
/// end
/// ```
#[derive(Debug, Clone, Default)]
pub struct MissingRespondToMissing;

impl MissingRespondToMissing {
    /// Whether a `def` node defines a singleton method (`def self.foo`),
    /// upstream's `:defs` node type.
    fn is_singleton(def: &Node<'_>) -> bool {
        def.as_def_node().is_some_and(|def| def.receiver().is_some())
    }

    /// Upstream's `node.parent.parent`, generalized for wrapping calls
    /// (e.g. `private def method_missing; end`, parsed as a `CallNode`
    /// whose sole `ArgumentsNode` argument is the `DefNode` -- an extra
    /// hop whitequark's send-node shape never introduces): the nearest
    /// enclosing `StatementsNode` among `ancestors` (searching from the
    /// innermost outward, so a directly-nested `def` finds its own
    /// immediate parent first) and *that* node's own parent, which is
    /// upstream's `grand_parent` regardless of how many non-`StatementsNode`
    /// wrappers (`CallNode`, `ArgumentsNode`, ...) sit between the `def`
    /// and its enclosing body.
    fn owning_scope(ancestors: &[linter::NodeInfo]) -> Option<linter::NodeInfo> {
        let index = ancestors.iter().rposition(|a| a.kind == NodeKind::StatementsNode)?;
        index.checked_sub(1).map(|i| ancestors[i])
    }

    /// Finds the node with the given `(kind, span)` inside `root`'s
    /// subtree. See the module doc's "Scope lookup" section: this
    /// re-derives the actual node [`Context::ancestors`] only reports as
    /// kind and span.
    fn node_at<'pr>(root: &Node<'pr>, kind: NodeKind, span: Span) -> Option<Node<'pr>> {
        if root.kind() == kind && root.span() == span {
            return Some(*root);
        }
        let mut found = None;
        ruby_ast::for_each_child(root, |child| {
            if found.is_none() {
                found = Self::node_at(child, kind, span);
            }
        });
        found
    }

    /// RuboCop's `implements_respond_to_missing?`, applied to an
    /// already-resolved owning scope. See the module doc's "Same-kind
    /// pairing" and "Nested search" sections.
    fn implements_respond_to_missing(scope: &Node<'_>, singleton: bool) -> bool {
        let mut found = false;
        ruby_ast::each_descendant(scope, &mut |descendant| {
            if found {
                return;
            }
            let Some(def) = descendant.as_def_node() else { return };
            if def.receiver().is_some() == singleton
                && def.name().as_slice() == b"respond_to_missing?"
            {
                found = true;
            }
        });
        found
    }
}

impl Rule for MissingRespondToMissing {
    const META: RuleMeta = RuleMeta {
        name: "Style/MissingRespondToMissing",
        department: Department::Style,
        summary: "Checks for the presence of `method_missing` without also defining `respond_to_missing?`.",
        explanation: "\
Checks for the presence of `method_missing` without also
defining `respond_to_missing?`.

Not defining `respond_to_missing?` will cause metaprogramming
methods like `respond_to?` to behave unexpectedly:

```ruby
class StringDelegator
  def initialize(string)
    @string = string
  end

  def method_missing(name, *args)
    @string.send(name, *args)
  end
end

delegator = StringDelegator.new(\"foo\")
# Claims to not respond to `upcase`.
delegator.respond_to?(:upcase) # => false
# But you can call it.
delegator.upcase # => FOO
```

```ruby
# bad
def method_missing(name, *args)
  if @delegate.respond_to?(name)
    @delegate.send(name, *args)
  else
    super
  end
end

# good
def respond_to_missing?(name, include_private)
  @delegate.respond_to?(name) || super
end

def method_missing(name, *args)
  if @delegate.respond_to?(name)
    @delegate.send(name, *args)
  else
    super
  end
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::DefNode],
        config: &[],
        blind_spots: "\
Upstream's `node.parent.parent` ancestor hop does not respect scope
boundaries, so a `respond_to_missing?` nested inside an unrelated nested
class/module within the same enclosing body still counts as implementing
it; this port reproduces that faithfully via `each_descendant` rather than
restricting the search to the immediate class/module body.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let def = node.as_def_node().expect("kind matched");
        if def.name().as_slice() != b"method_missing" {
            return;
        }
        let singleton = Self::is_singleton(node);

        let implements = Self::owning_scope(ctx.ancestors())
            .and_then(|scope| Self::node_at(&ctx.parsed().root(), scope.kind, scope.span))
            .is_some_and(|scope| Self::implements_respond_to_missing(&scope, singleton));

        if !implements {
            ctx.report(&Self::META, node.span(), MSG);
        }
    }
}
