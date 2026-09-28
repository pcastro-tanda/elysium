//! `Lint/ReturnInVoidContext`, ported from RuboCop's
//! `lib/rubocop/cop/lint/return_in_void_context.rb`.
//!
//! # Ancestor tracking
//!
//! Upstream walks two independent ancestor chains from each `return`: the
//! nearest enclosing `any_def` (`:def` or `:defs`, i.e. either an instance or
//! a singleton method definition) to test `void_context?`, and *every*
//! enclosing `any_block` (a literal block, numblock, itblock, or stabby
//! lambda) to test whether any of them is owned by a
//! [`SCOPE_CHANGING_METHODS`]-named call. [`Context::ancestors`] only carries
//! kind and span, not a `DefNode`'s name/receiver or a block's owning call's
//! name, so this port keeps its own stacks instead: `def_stack`, pushed with
//! a precomputed `void_context`/method name on every [`NodeKind::DefNode`]
//! and popped on leaving it, and `block_stack`, pushed with whether the
//! block is scope-changing on every literal-block-owning [`NodeKind::CallNode`]
//! *and* every [`NodeKind::LambdaNode`] (Prism's distinct node kind for a
//! stabby `-> { }`, which whitequark instead represents as an ordinary
//! `:block` node whose `send` is named `:lambda` -- so this port pushes the
//! same "scope-changing" flag directly for it, without a synthetic call).
//! `block_stack.iter().any(...)` mirrors upstream's unbounded
//! `each_ancestor(:any_block).any?`: neither side restricts the search to
//! blocks between the `return` and its enclosing `def`.
//!
//! # `void_context?`
//!
//! `DefNode#void_context?` is `(def_type? && method?(:initialize)) ||
//! assignment_method?`. `def_type?` is only true for a `:def` node (an
//! instance method, or a method defined inside `class << self`), never a
//! `:defs` node (`def self.foo`); Prism represents both as the same
//! `DefNode` kind, distinguished by an optional `receiver` field, so this
//! port checks `receiver().is_none()` in its place.
//! `assignment_method?` (`method_name.end_with?('=') &&
//! !comparison_method?`) does *not* check `def_type?`, so a singleton setter
//! (`def self.foo=`) is void context too -- deliberately mirrored here, not
//! a bug.
//!
//! # Value check
//!
//! `return_node.descendants.any?` (true only when `return` carries at least
//! one value) becomes `ReturnNode::arguments().is_some()`: a bare `return`
//! (or `return if cond`, where the modifier `if` is a parent, not a
//! descendant, of the `return` node) has no `arguments`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// One enclosing `def`, computed once on entry and reused for every
/// `return` found in its body (including in nested blocks and lambdas).
#[derive(Debug, Clone, Copy)]
struct DefFrame {
    /// RuboCop-AST's `DefNode#void_context?`.
    void_context: bool,
    /// Span of the def's own name (`name_loc`), e.g. `initialize` or
    /// `foo=`, read via `ctx.text` only on the report path.
    name_span: Span,
}

/// RuboCop-AST's `MethodIdentifierPredicates#assignment_method?`:
/// `method_name.end_with?('=') && !comparison_method?`, where
/// `comparison_method?` is `Node::COMPARISON_OPERATORS.include?(method_name)`
/// (`==`, `===`, `!=`, `<=`, `>=`, `>`, `<`; only the first five end in `=`
/// and are relevant here).
fn is_assignment_method(name: &[u8]) -> bool {
    name.ends_with(b"=") && !matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=")
}

/// RuboCop's `SCOPE_CHANGING_METHODS`: returning out of these only exits the
/// block itself, not the enclosing method.
fn is_scope_changing_call(name: &[u8]) -> bool {
    matches!(name, b"lambda" | b"define_method" | b"define_singleton_method")
}

/// Whether a `CallNode` owns a literal block (as opposed to a `&block`
/// argument pass, which is a `BlockArgumentNode` and does not count as
/// `any_block_type?` upstream).
fn owns_literal_block(node: &Node<'_>) -> bool {
    node.as_call_node()
        .is_some_and(|call| call.block().is_some_and(|b| b.as_block_node().is_some()))
}

/// Checks for the use of a return with a value in a context where the value
/// will be ignored (`initialize` and setter methods).
///
/// # Examples
///
/// ```ruby
/// # bad
/// def initialize
///   foo
///   return :qux if bar?
///   baz
/// end
///
/// def foo=(bar)
///   return 42
/// end
///
/// # good
/// def initialize
///   foo
///   return if bar?
///   baz
/// end
///
/// def foo=(bar)
///   return
/// end
/// ```
#[derive(Debug, Clone, Default)]
pub struct ReturnInVoidContext {
    /// Enclosing `def`s, innermost last.
    def_stack: Vec<DefFrame>,
    /// Whether each enclosing block/lambda (in `each_ancestor(:any_block)`'s
    /// sense) is owned by a scope-changing call, innermost last.
    block_stack: Vec<bool>,
}

impl ReturnInVoidContext {
    fn check(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let ret = node.as_return_node().expect("kind matched");
        if ret.arguments().is_none() {
            return;
        }
        let Some(def_frame) = self.def_stack.last() else {
            return;
        };
        if !def_frame.void_context {
            return;
        }
        if self.block_stack.iter().any(|&scope_changing| scope_changing) {
            return;
        }
        let method = String::from_utf8_lossy(ctx.text(def_frame.name_span));
        ctx.report(
            &Self::META,
            ret.keyword_loc().span(),
            format!("Do not return a value in `{method}`."),
        );
    }
}

impl Rule for ReturnInVoidContext {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ReturnInVoidContext",
        department: Department::Lint,
        summary: "Checks for the use of a return with a value in a context where the value will be ignored.",
        explanation: "\
Checks for the use of a return with a value in a context
where the value will be ignored. (initialize and setter methods)

```ruby
# bad
def initialize
  foo
  return :qux if bar?
  baz
end

def foo=(bar)
  return 42
end

# good
def initialize
  foo
  return if bar?
  baz
end

def foo=(bar)
  return
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode, NodeKind::ReturnNode, NodeKind::CallNode, NodeKind::LambdaNode],
        config: &[],
        blind_spots: "\
`assignment_method?` does not check `def_type?`, so a singleton setter
(`def self.foo=`) is void context too, exactly like an instance setter --
mirrored here rather than treated as a bug. `each_ancestor(:any_block)` is
unbounded: a scope-changing block/lambda *outside* the enclosing `def`
(which cannot normally contain a `return` reaching back into it) is, like
upstream, never distinguished from one directly wrapping the `return`.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.def_stack.clear();
        self.block_stack.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode => {
                let def = node.as_def_node().expect("kind matched");
                let name = def.name();
                let name = name.as_slice();
                let void_context = (def.receiver().is_none() && name == b"initialize")
                    || is_assignment_method(name);
                let name_span = def.name_loc().span();
                self.def_stack.push(DefFrame { void_context, name_span });
            }
            NodeKind::CallNode => {
                if owns_literal_block(node) {
                    let call = node.as_call_node().expect("kind matched");
                    self.block_stack.push(is_scope_changing_call(call.name().as_slice()));
                }
            }
            NodeKind::LambdaNode => {
                self.block_stack.push(true);
            }
            NodeKind::ReturnNode => {
                self.check(node, ctx);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode => {
                self.def_stack.pop();
            }
            NodeKind::CallNode => {
                if owns_literal_block(node) {
                    self.block_stack.pop();
                }
            }
            NodeKind::LambdaNode => {
                self.block_stack.pop();
            }
            _ => {}
        }
    }
}
