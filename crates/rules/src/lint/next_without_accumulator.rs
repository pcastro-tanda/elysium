//! `Lint/NextWithoutAccumulator`, ported from RuboCop's
//! `lib/rubocop/cop/lint/next_without_accumulator.rb`.
//!
//! # `on_block_body_of_reduce` node pattern
//!
//! Upstream's node pattern is
//! `{ (block (call _recv {:reduce :inject} !sym) _blockargs $(begin ...)) (numblock ...) }`.
//! Two clauses of this are load-bearing beyond the obvious "receiver calls
//! `reduce`/`inject`":
//!
//! - `!sym` occupies the call's *only* argument position: the whole pattern
//!   requires the call to carry **exactly one** argument, and that argument
//!   must not be a literal symbol. [`has_single_non_symbol_argument`] mirrors
//!   this exactly: a bare `.reduce { ... }` (no initial-value argument) or a
//!   `.reduce(:+)`-shaped call (irrelevant here since that form never
//!   actually carries a block, but syntactically parseable) never matches,
//!   so neither is ever analyzed by this cop -- matching upstream.
//! - `$(begin ...)` requires the block's *whole body* to itself be a `begin`
//!   node, which whitequark's parser only produces when a body holds two or
//!   more statements (a single-statement body is that statement directly,
//!   un-wrapped). Prism has no such special case -- `BlockNode::body` is
//!   always a `StatementsNode`, holding one statement or several alike (see
//!   `Lint/ConstantDefinitionInBlock`'s module doc for the same observation)
//!   -- so this port reproduces the quirk explicitly: a qualifying block's
//!   body must hold at least two statements, or the block is never analyzed
//!   at all, even when its lone statement is itself a bare `next`.
//!
//! `alias on_numblock on_block` covers `_1`-style implicit params; Prism
//! represents a numbered-parameter block with the very same `BlockNode` kind
//! as an ordinary `|acc, i|` block (they only differ in what `parameters()`
//! points at, which this cop never inspects), so no extra handling is
//! needed for the numblock fixtures. Upstream, however, does *not* alias
//! `on_itblock` (Ruby 3.4's bare-`it`-parameter block) -- so
//! `(1..4).reduce(0) { next if it.odd? }` is never flagged upstream. Since
//! Prism likewise represents an itblock as an ordinary `BlockNode`
//! indistinguishable from the other two, this port flags it too: a
//! deliberate, harmless (no fixture exercises it) broadening beyond
//! upstream.
//!
//! # `parent_block_node`
//!
//! Upstream's `node.each_ancestor(:any_block).first` walks all the way up
//! from a candidate `next` node, through any intervening node kind
//! (including a nested `def`, which would itself make `next` invalid at
//! runtime, but is not special-cased here either), until it reaches the
//! nearest enclosing block. [`find_void_next`] mirrors that unrestricted
//! walk directly: it descends into every descendant of the block's body
//! *except* a nested [`ruby_ast::NodeKind::BlockNode`] (matching
//! `any_block_type?`, which stops the ancestor walk one level too early to
//! ever reach a `next` that is actually inside a deeper block), and reports
//! only the first bare `next` (no explicit arguments) it finds in that
//! walk, exactly as `each_node(:next).find(...)` yields only the first
//! match in document order.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Use `next` with an accumulator argument in a `reduce`.";

/// Don't omit the accumulator when calling `next` in a `reduce` block.
#[derive(Debug, Clone)]
pub struct NextWithoutAccumulator;

impl Rule for NextWithoutAccumulator {
    const META: RuleMeta = RuleMeta {
        name: "Lint/NextWithoutAccumulator",
        department: Department::Lint,
        summary: "Don't omit the accumulator when calling `next` in a `reduce` block.",
        explanation: "\
Don't omit the accumulator when calling `next` in a `reduce` block.

```ruby
# bad
result = (1..4).reduce(0) do |acc, i|
  next if i.odd?
  acc + i
end

# good
result = (1..4).reduce(0) do |acc, i|
  next acc if i.odd?
  acc + i
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Only fires when the `reduce`/`inject` call carries exactly one, non-symbol
argument (the initial accumulator value) -- a bare `.reduce { ... }` with no
initial value, or the symbol-operator form `.reduce(:+)`, is never analyzed,
matching upstream's own node-pattern arity requirement. The block's body
must hold at least two statements before it is analyzed at all -- a block
whose only statement is a bare `next` (e.g. `.reduce(0) { |acc, i| next if
i.odd? }`) is never flagged, reproducing a quirk of whitequark's parser
(which only wraps multi-statement bodies in a `begin` node, the shape
upstream's own pattern explicitly requires) that has no Prism equivalent to
suppress. Unlike upstream (which aliases `on_block`/`on_numblock` but not
`on_itblock`), a bare-`it`-parameter block is flagged here too, since Prism
represents all three block forms identically. A `next` nested inside a
`def` within the block (not itself inside a further nested block) is still
matched against the outer block, mirroring upstream's own ancestor walk,
which does not stop at `def`/`class` boundaries either.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if !matches!(call.name().as_slice(), b"reduce" | b"inject") {
            return;
        }
        if !has_single_non_symbol_argument(&call) {
            return;
        }
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        let Some(body) = block.body() else { return };
        let Some(statements) = body.as_statements_node() else { return };
        if statements.body().len() < 2 {
            return;
        }
        if let Some(void_next) = find_void_next(&body) {
            ctx.report(&Self::META, void_next.span(), MSG);
        }
    }
}

/// The `!sym`-occupied-single-argument-position half of upstream's
/// `on_block_body_of_reduce` node pattern. See the module doc.
fn has_single_non_symbol_argument(call: &CallNode<'_>) -> bool {
    let Some(arguments) = call.arguments() else { return false };
    let arguments = arguments.arguments();
    if arguments.len() != 1 {
        return false;
    }
    let argument = arguments.first().expect("len checked above");
    argument.kind() != NodeKind::SymbolNode
}

/// RuboCop's `body.each_node(:next).find { |n| n.children.empty? &&
/// parent_block_node(n) == node }`: the first bare `next` reachable from
/// `node` without descending into a nested block. See the module doc.
fn find_void_next<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    if let Some(next) = node.as_next_node() {
        if next.arguments().is_none() {
            return Some(*node);
        }
        return None;
    }
    if node.kind() == NodeKind::BlockNode {
        return None;
    }
    let mut found = None;
    for_each_child(node, |child| {
        if found.is_none() {
            found = find_void_next(child);
        }
    });
    found
}
