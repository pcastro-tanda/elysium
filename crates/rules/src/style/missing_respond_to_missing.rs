//! `Style/MissingRespondToMissing`, ported from RuboCop's
//! `lib/rubocop/cop/style/missing_respond_to_missing.rb`.
//!
//! # Scope tracking
//!
//! Upstream's `implements_respond_to_missing?` reaches the def's owning
//! scope via `node.parent.parent`, then does one `each_descendant` walk of
//! that scope per `method_missing` def. Re-deriving the owning scope from
//! the parsed tree's root and re-walking its subtree for every
//! `method_missing` def would be quadratic in pathological files with many
//! such defs; instead, [`MissingRespondToMissing`] keeps a stack of
//! [`ScopeFrame`]s, pushed in [`Rule::enter`]/popped in [`Rule::leave`] for
//! `ClassNode`/`ModuleNode`/`SingletonClassNode`/`ProgramNode` (the only
//! node kinds a `method_missing`/`respond_to_missing?` pair can be scoped
//! by). Each frame's `respond_to_missing?` presence (instance and
//! singleton, tracked separately per the "Same-kind pairing" section below)
//! is computed once, by a single `each_descendant` walk of the scope's own
//! subtree at `enter` time -- so a `method_missing` def only ever does an
//! `O(1)` lookup against the already-computed top frame, no matter how
//! many such defs the scope contains.
//!
//! # Scope lookup and the `begin`-elision quirk
//!
//! whitequark only wraps a *multi*-statement body in a `begin` node; a
//! lone statement is the class/module/singleton-class node's `body` value
//! directly, with no intervening wrapper. So for a `method_missing` that is
//! the *only* statement in its immediately enclosing scope, `node.parent`
//! (the first hop) lands on that scope node itself (no `begin` to skip),
//! and the second hop, `node.parent.parent`, lands on *the scope's own
//! parent* -- one level further out than the immediately enclosing scope.
//! A `private def method_missing; end` still needs only that same single
//! first hop, since whitequark's `send` node holds the wrapped `def`
//! directly (no extra wrapper for its argument list) and itself occupies
//! the one body slot.
//!
//! Prism always wraps a body in a `StatementsNode` regardless of statement
//! count (see `Lint::ConstantDefinitionInBlock`'s module doc for the
//! general observation), so [`ScopeFrame::new`] instead records the
//! *length* of the scope's own immediate body (`body_len`): when a
//! `method_missing`'s immediately enclosing frame has `body_len == 1`, its
//! search target becomes that frame's *enclosing* frame (one level further
//! out on the stack) rather than the frame itself, reproducing the extra
//! hop directly. No deeper, cascading elision is attempted (checking
//! whether *that* enclosing frame is itself a lone statement of its own
//! parent, and so on): a single hop already lands on a search target
//! covering the exact same subtree either way. Concretely, if the
//! enclosing frame's own body has more than one statement, `node.parent`
//! (from the lone-statement scope) is the `begin` node wrapping *that*
//! body, whose descendants are exactly the enclosing scope's own children;
//! if the enclosing frame's body also has exactly one statement (itself
//! elided), `node.parent` is the enclosing scope node directly. Either way
//! the descendant search covers precisely the enclosing frame's subtree,
//! which is exactly [`ScopeFrame::new`]'s own precomputed result for that
//! frame -- so no cascading is ever needed:
//!
//! ```ruby
//! class A
//!   def method_missing; end
//! end
//!
//! class B
//!   def respond_to_missing?; end
//! end
//! ```
//!
//! is *not* flagged upstream: `A`'s `method_missing` is its class body's
//! only statement, so the search skips past `A` entirely to the top-level
//! program, whose descendants include `B`'s `respond_to_missing?`. This
//! port reproduces that skip via the `body_len == 1` check above.
//!
//! When the immediately enclosing frame is already the outermost one (the
//! top-level program itself -- a `method_missing` def written directly at
//! the top level, not nested in any class/module), there is no further
//! frame to skip to, so this port keeps searching the program's own
//! subtree regardless of `body_len`; upstream's `node.parent.parent` would
//! actually be `nil` in that position (there is nothing above the
//! program), always failing the "implements" check and always flagging,
//! which this port does not reproduce -- a narrow, pre-existing blind spot
//! for bare top-level `method_missing` defs (as opposed to ones nested in
//! a class/module/singleton class), which is not covered by any fixture.
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

/// RuboCop's `MSG`.
const MSG: &str = "When using `method_missing`, define `respond_to_missing?`.";

/// One `ClassNode`/`ModuleNode`/`SingletonClassNode`/`ProgramNode` scope's
/// precomputed `respond_to_missing?` presence, plus its own immediate body
/// length (used to detect the `begin`-elision quirk described in the
/// module doc's "Scope lookup" section).
#[derive(Debug, Clone, Copy)]
struct ScopeFrame {
    /// The number of statements in this scope's own immediate body (`0` for
    /// an empty body).
    body_len: usize,
    /// Whether an instance `respond_to_missing?` def exists anywhere in
    /// this scope's subtree.
    has_instance: bool,
    /// Whether a singleton `respond_to_missing?` def exists anywhere in
    /// this scope's subtree.
    has_singleton: bool,
}

impl ScopeFrame {
    /// Computes a scope's frame with a single `each_descendant` walk of
    /// `node`'s subtree.
    fn new(node: &Node<'_>) -> Self {
        let mut has_instance = false;
        let mut has_singleton = false;
        ruby_ast::each_descendant(node, &mut |descendant| {
            let Some(def) = descendant.as_def_node() else { return };
            if def.name().as_slice() != b"respond_to_missing?" {
                return;
            }
            if def.receiver().is_some() {
                has_singleton = true;
            } else {
                has_instance = true;
            }
        });
        Self { body_len: scope_body_len(node), has_instance, has_singleton }
    }

    /// Whether this scope implements a `respond_to_missing?` of the given
    /// kind (`true` for singleton, `false` for instance).
    fn implements(&self, singleton: bool) -> bool {
        if singleton {
            self.has_singleton
        } else {
            self.has_instance
        }
    }
}

/// The number of statements in a `ClassNode`/`ModuleNode`/
/// `SingletonClassNode`'s own immediate body, or a `ProgramNode`'s own
/// top-level statements. `0` for an empty (or absent) body.
fn scope_body_len(node: &Node<'_>) -> usize {
    let body = match node {
        Node::ProgramNode { .. } => {
            return node.as_program_node().map_or(0, |n| n.statements().body().len());
        }
        Node::ClassNode { .. } => node.as_class_node().and_then(|n| n.body()),
        Node::ModuleNode { .. } => node.as_module_node().and_then(|n| n.body()),
        Node::SingletonClassNode { .. } => node.as_singleton_class_node().and_then(|n| n.body()),
        _ => None,
    };
    body.and_then(|b| b.as_statements_node()).map_or(0, |s| s.body().len())
}

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
pub struct MissingRespondToMissing {
    /// The stack of enclosing `ClassNode`/`ModuleNode`/`SingletonClassNode`/
    /// `ProgramNode` scopes, innermost last. `ProgramNode` is always pushed
    /// first (it is the parsed tree's root), so this is never empty while
    /// visiting any other node.
    scopes: Vec<ScopeFrame>,
}

impl MissingRespondToMissing {
    /// Whether a `def` node defines a singleton method (`def self.foo`),
    /// upstream's `:defs` node type.
    fn is_singleton(def: &Node<'_>) -> bool {
        def.as_def_node().is_some_and(|def| def.receiver().is_some())
    }

    /// Whether `node` opens one of the scope kinds this cop tracks.
    fn is_scope(node: &Node<'_>) -> bool {
        matches!(
            node.kind(),
            NodeKind::ClassNode
                | NodeKind::ModuleNode
                | NodeKind::SingletonClassNode
                | NodeKind::ProgramNode
        )
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
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ClassNode,
            NodeKind::ModuleNode,
            NodeKind::SingletonClassNode,
            NodeKind::ProgramNode,
            NodeKind::DefNode,
        ],
        config: &[],
        blind_spots: "\
Upstream's `node.parent.parent` ancestor hop does not respect scope
boundaries, so a `respond_to_missing?` nested inside an unrelated nested
class/module within the same enclosing body still counts as implementing
it; this port reproduces that faithfully via `each_descendant` rather than
restricting the search to the immediate class/module body. A
`method_missing` that is its immediately enclosing scope's only statement
searches that scope's own enclosing scope instead (matching whitequark's
`begin`-elision quirk, see the module doc's \"Scope lookup\" section) --
except when that immediately enclosing scope is already the top-level
program (a bare top-level `method_missing`, not nested in any
class/module/singleton class), where this port keeps searching the
program's own subtree rather than always flagging as upstream would.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if Self::is_scope(node) {
            self.scopes.push(ScopeFrame::new(node));
            return;
        }
        let Some(def) = node.as_def_node() else { return };
        if def.name().as_slice() != b"method_missing" {
            return;
        }
        let singleton = Self::is_singleton(node);

        // The top of the stack is this def's immediately enclosing scope
        // (`ProgramNode` is always pushed first, so the stack is never
        // empty here). See the module doc's "Scope lookup" section for the
        // `body_len == 1` skip.
        let top = self.scopes.len() - 1;
        let search_index = if self.scopes[top].body_len == 1 && top > 0 { top - 1 } else { top };

        if !self.scopes[search_index].implements(singleton) {
            ctx.report(&Self::META, node.span(), MSG);
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if Self::is_scope(node) {
            self.scopes.pop();
        }
    }
}
