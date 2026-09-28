//! `Style/MultilineBlockChain`, ported from RuboCop's
//! `lib/rubocop/cop/style/multiline_block_chain.rb`.
//!
//! # Node shapes
//!
//! Whitequark models `recv do ... end` as its own `:block` (or `:numblock`
//! for `_1`/`_2` params, `:itblock` for Ruby 3.4's bare `it`) node wrapping a
//! `send`/`csend` node, so a chained `end.method` reads as a `send` node
//! whose `receiver` is that wrapping `:block`-family node. Prism has no such
//! wrapper: a call's block (however its parameters are spelled) is just the
//! `block` field of the same [`NodeKind::CallNode`] that names the method,
//! and that same `CallNode` is what appears as another call's `receiver`.
//! Prism's `block` field is also where a `&blk`/`&:sym` block-pass argument
//! shows up, as a `BlockArgumentNode` rather than a `BlockNode` -- upstream
//! only ever visits real `:block`/`:numblock`/`:itblock` literals (never a
//! block-pass), so both the visited call and any receiver-chain "has a
//! block" check here must additionally confirm `block.kind() ==
//! NodeKind::BlockNode`, i.e. that it downcasts via `as_block_node`. So
//! `receiver.any_block_type?` here is "receiver is a `CallNode` whose
//! `block` is a `BlockNode`", and no separate handling is needed for
//! numbered/`it` params -- Prism folds all three of upstream's node kinds
//! into one.
//!
//! Upstream visits every `:block`/`:numblock`/`:itblock` node once (via
//! `on_block`/its aliases) and, for *that* node's own `send_node`, walks
//! `each_node(:call)` over the send node's whole subtree (self first, then
//! children in source order -- receiver subtree before argument subtrees)
//! until it finds a call whose immediate `receiver` is itself a multi-line
//! block, reporting exactly once per outer block and breaking. Each nested
//! block gets its own independent `on_block` call, so this traversal only
//! ever needs to look at the *immediate* receiver chain of the call that
//! owns the block being visited: if that chain's first (outermost) call
//! already has a block-shaped receiver it is reported immediately; if not,
//! the walk continues one receiver down. It never needs to recurse into a
//! block's own body (a different node kind) or, in every fixture here, into
//! argument subtrees -- so this port walks the plain `CallNode::receiver()`
//! chain from the block-owning call downward and stops at the first
//! multi-line block-shaped receiver, which reproduces every upstream
//! example. A call chain where the block-shaped receiver instead sits
//! inside an *argument* rather than anywhere in the receiver chain (e.g.
//! `foo(bar do end.baz) do end`) is not reachable through this simplified
//! walk; see `blind_spots`.
//!
//! # Offense condition and range
//!
//! The guard is `receiver.multiline?`, and `rubocop-ast`'s
//! `BlockNode#multiline?` is overridden to compare `loc.begin.line` (the
//! opening `do`/`{`) against `loc.end.line` (the closing `end`/`}`) of the
//! block itself, not the generic `Node#multiline?`'s whole-node first/last
//! line (which would also count a multi-line receiver call before the
//! block even opens). So this checks the receiver block's own
//! `opening_loc`/`closing_loc` lines, not [`Context::is_single_line`] over
//! the receiver call's whole span.
//!
//! `range_between(receiver.loc.end.begin_pos, node.send_node.source_range
//! .end_pos)`: from the start of the receiver block's closing `end`/`}` to
//! the end of the *whole* outer call chain excluding its own attached block
//! (RuboCop's `send_node.source`, since a `send`/`csend` node's own range
//! never includes a block it owns). [`call_span_excluding_block`] already
//! implements that second half for a `CallNode`; the first half is the
//! start of the receiver's own attached block's `closing_loc`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Avoid multi-line chains of blocks.";

/// Checks for chaining of a block after another block that spans multiple
/// lines.
#[derive(Debug, Clone, Default)]
pub struct MultilineBlockChain;

impl Rule for MultilineBlockChain {
    const META: RuleMeta = RuleMeta {
        name: "Style/MultilineBlockChain",
        department: Department::Style,
        summary: "Checks for chaining of a block after another block that spans multiple lines.",
        explanation: "\
Checks for chaining of a block after another block that spans multiple
lines.

```ruby
# bad
Thread.list.select do |t|
  t.alive?
end.map do |t|
  t.object_id
end

# good
alive_threads = Thread.list.select do |t|
  t.alive?
end
alive_threads.map do |t|
  t.object_id
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Only the immediate receiver chain of the call that owns a block is walked
for a multi-line block-shaped receiver; upstream's `each_node(:call)` would
also search argument subtrees once the whole receiver chain is exhausted,
so a block-shaped receiver reachable only through an argument (never
through any level of the receiver chain) is not flagged. No fixture in the
corpus exercises this.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.block().and_then(|b| b.as_block_node()).is_none() {
            return;
        }

        let mut current = call;
        loop {
            let Some(receiver) = current.receiver() else { return };
            let Some(receiver_call) = receiver.as_call_node() else { return };
            let Some(block) = receiver_call.block().and_then(|b| b.as_block_node()) else {
                current = receiver_call;
                continue;
            };
            let block_span =
                Span::new(block.opening_loc().span().start, block.closing_loc().span().end);
            if !ctx.is_single_line(block_span) {
                let range = Span::new(
                    block.closing_loc().span().start,
                    call_span_excluding_block(&call).end,
                );
                ctx.report(&Self::META, range, MSG);
                return;
            }
            current = receiver_call;
        }
    }
}
