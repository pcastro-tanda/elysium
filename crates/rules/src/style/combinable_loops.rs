//! `Style/CombinableLoops`, ported from RuboCop's
//! `lib/rubocop/cop/style/combinable_loops.rb`.
//!
//! # Node shapes
//!
//! Whitequark's `on_block`/`on_numblock`/`on_itblock` all fire on one `block`
//! AST node type (they differ only in how it spells its middle "arguments"
//! child); Prism folds a plain block, a numbered block (`_1`), and an
//! `it`-block into the same [`ruby_ast::node::BlockNode`] kind, so this port
//! needs no separate handling for any of the three. Unlike whitequark, Prism
//! never gives a `BlockNode` its own place in a `StatementsNode`'s body list
//! -- the *owning* [`ruby_ast::node::CallNode`] is the statement, with the
//! block attached to its `block` field -- so [`CombinableLoops::enter`]
//! (subscribed to [`NodeKind::StatementsNode`]) walks each body list looking
//! for a `CallNode` whose `block()` is a literal [`ruby_ast::node::BlockNode`]
//! (never a `BlockArgumentNode`, i.e. `&blk`, which is never itself the
//! attached block of a *previous* statement either). This also reproduces
//! upstream's `node.parent&.begin_type?` guard for free: only a statement
//! that is itself a direct element of some `StatementsNode` is ever
//! considered.
//!
//! `for` loops need no such indirection: Prism's `ForNode` already is the
//! whole statement, with its own `statements` field standing in for
//! whitequark's `body`.
//!
//! # Structural equality
//!
//! `receiver ==`, `arguments ==`, `collection ==`, and `index ==`/block
//! `arguments ==` are RuboCop's generic `Parser::AST::Node#==` (deep
//! structural equality); this port approximates all of them with exact
//! source-text comparison, the same approximation `Lint::SelfAssignment` and
//! `Style::RedundantCondition` use for the same problem. A numbered or
//! `it`-block's implicit parameter list has no source text of its own (its
//! location is empty), so two of the same flavour always compare equal --
//! matching upstream, where `numblock`/`itblock` never carry an explicit,
//! possibly-differing parameter list either.
//!
//! # Autocorrection
//!
//! [`combine_with_left_sibling`] merges the current statement's body into
//! its left sibling's by deleting the left sibling's own trailing
//! close-token (`combine_with_left_sibling`'s first edit) and the current
//! statement's own header up through its body's start (its second edit),
//! leaving the two bodies adjacent, separated only by whatever source text
//! (typically a newline) sat between the two statements already.
//! [`correct_end_of_block`] then additionally rewrites the current
//! statement's own closing token to match its left sibling's brace style --
//! but only when the *next* statement is not itself a block-loop statement:
//! deferring lets a run of three or more combinable loops correct their
//! shared closing token exactly once, on the last pair's own offense.
//! `for` loops have no brace-vs-`do`/`end` choice, so
//! [`correct_end_of_block`] is skipped entirely for that combination
//! (mirroring upstream's `respond_to?(:braces?)` guard, true only for a
//! block).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ArgumentsNode, BlockNode, CallNode, ForNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Combine this loop with the previous loop.";

/// Checks for places where multiple consecutive loops over the same data can be combined into a single loop.
#[derive(Debug, Clone)]
pub struct CombinableLoops;

impl Rule for CombinableLoops {
    const META: RuleMeta = RuleMeta {
        name: "Style/CombinableLoops",
        department: Department::Style,
        summary: "Checks for places where multiple consecutive loops over the same data can be combined into a single loop.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(stmts) = node.as_statements_node() else { return };
        let body: Vec<Node<'_>> = stmts.body().iter().collect();
        for i in 1..body.len() {
            check_pair(ctx, &body[i - 1], &body[i], body.get(i + 1));
        }
    }
}

/// RuboCop's `on_block` (aliased `on_numblock`/`on_itblock`) and `on_for`,
/// applied to one adjacent statement pair from a `StatementsNode` body.
fn check_pair(ctx: &mut Context<'_>, prev: &Node<'_>, cur: &Node<'_>, next: Option<&Node<'_>>) {
    if let Some(cur_call) = cur.as_call_node() {
        if let Some(cur_block) = cur_call.block().and_then(|b| b.as_block_node()) {
            check_block_pair(ctx, prev, &cur_call, &cur_block, cur, next);
        }
    } else if let Some(cur_for) = cur.as_for_node() {
        check_for_pair(ctx, prev, &cur_for, cur);
    }
}

/// RuboCop's `collection_looping_method?`.
fn is_collection_looping_method(name: &[u8]) -> bool {
    name.starts_with(b"each") || name.ends_with(b"_each")
}

/// RuboCop's generic `Node#==`, restricted to the `nil`/real-node shapes
/// `receiver ==`/block `arguments ==` ever compare, approximated by source
/// text (see the module doc).
fn same_optional_node(ctx: &Context<'_>, a: Option<Node<'_>>, b: Option<Node<'_>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => ctx.text(a.span()) == ctx.text(b.span()),
        _ => false,
    }
}

/// RuboCop's `send_node.arguments == send_node.arguments`, applied to the
/// looping call's own arguments (e.g. `each_slice(2)`'s `2`), not the
/// block's parameters.
fn same_call_arguments(
    ctx: &Context<'_>,
    a: Option<ArgumentsNode<'_>>,
    b: Option<ArgumentsNode<'_>>,
) -> bool {
    same_optional_node(ctx, a.map(|n| n.as_node()), b.map(|n| n.as_node()))
}

/// RuboCop's `node.arguments == node.left_sibling.arguments`, applied to the
/// block's own parameter list (e.g. `|item|`), gating autocorrection only --
/// a mismatch still registers the offense, just without a fix.
fn same_block_parameters(ctx: &Context<'_>, a: Option<Node<'_>>, b: Option<Node<'_>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) if a.kind() == b.kind() => match a.kind() {
            NodeKind::NumberedParametersNode => a.as_numbered_parameters_node().is_some_and(|a| {
                b.as_numbered_parameters_node().is_some_and(|b| a.maximum() == b.maximum())
            }),
            NodeKind::ItParametersNode => true,
            _ => ctx.text(a.span()) == ctx.text(b.span()),
        },
        _ => false,
    }
}

/// RuboCop's `same_collection_looping_block?` plus its `on_block` callers'
/// own guards, folded into one check since this port reaches both from the
/// same [`check_pair`] dispatch.
fn check_block_pair(
    ctx: &mut Context<'_>,
    prev: &Node<'_>,
    cur_call: &CallNode<'_>,
    cur_block: &BlockNode<'_>,
    cur: &Node<'_>,
    next: Option<&Node<'_>>,
) {
    if !is_collection_looping_method(cur_call.name().as_slice()) {
        return;
    }
    let Some(prev_call) = prev.as_call_node() else { return };
    let Some(prev_block) = prev_call.block().and_then(|b| b.as_block_node()) else { return };
    if prev_call.name().as_slice() != cur_call.name().as_slice() {
        return;
    }
    if !same_optional_node(ctx, prev_call.receiver(), cur_call.receiver()) {
        return;
    }
    if !same_call_arguments(ctx, prev_call.arguments(), cur_call.arguments()) {
        return;
    }
    let (Some(prev_body), Some(cur_body)) = (prev_block.body(), cur_block.body()) else { return };

    let span = cur.span();
    if same_block_parameters(ctx, prev_block.parameters(), cur_block.parameters()) {
        let mut edits = vec![
            Edit::delete(Span::new(prev_body.span().end, prev_block.closing_loc().span().end)),
            Edit::delete(Span::new(cur.span().start, cur_body.span().start)),
        ];
        correct_end_of_block(ctx, &prev_block, cur_block, next, &mut edits);
        ctx.report_with_fix(
            &CombinableLoops::META,
            span,
            MSG,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    } else {
        ctx.report(&CombinableLoops::META, span, MSG);
    }
}

/// RuboCop's `same_collection_looping_for?` plus its `on_for` caller's own
/// guards.
fn check_for_pair(ctx: &mut Context<'_>, prev: &Node<'_>, cur_for: &ForNode<'_>, cur: &Node<'_>) {
    let Some(prev_for) = prev.as_for_node() else { return };
    if ctx.text(prev_for.collection().span()) != ctx.text(cur_for.collection().span()) {
        return;
    }
    let (Some(prev_stmts), Some(cur_stmts)) = (prev_for.statements(), cur_for.statements()) else {
        return;
    };

    let span = cur.span();
    if ctx.text(prev_for.index().span()) == ctx.text(cur_for.index().span()) {
        let edits = vec![
            Edit::delete(Span::new(
                prev_stmts.as_node().span().end,
                prev_for.end_keyword_loc().span().end,
            )),
            Edit::delete(Span::new(cur.span().start, cur_stmts.as_node().span().start)),
        ];
        ctx.report_with_fix(
            &CombinableLoops::META,
            span,
            MSG,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    } else {
        ctx.report(&CombinableLoops::META, span, MSG);
    }
}

/// A statement that is itself a call with a literal block attached --
/// RuboCop's `any_block_type?`, applied to whatever [`check_pair`] found as
/// the *next* statement (regardless of whether it would itself qualify as a
/// combinable loop): upstream's `correct_end_of_block` defers the closing
/// token rewrite whenever one follows, so a run of three or more loops only
/// rewrites it once, on the last pair's own offense.
fn is_block_loop_statement(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|c| c.block().is_some_and(|b| b.as_block_node().is_some()))
}

/// RuboCop's `correct_end_of_block`.
fn correct_end_of_block(
    ctx: &Context<'_>,
    prev_block: &BlockNode<'_>,
    cur_block: &BlockNode<'_>,
    next: Option<&Node<'_>>,
    edits: &mut Vec<Edit>,
) {
    if next.is_some_and(is_block_loop_statement) {
        return;
    }
    let end_of_block: &[u8] =
        if ctx.text(prev_block.closing_loc().span()) == b"}" { b"}" } else { b" end" };
    edits.push(Edit::replace(cur_block.closing_loc().span(), end_of_block.to_vec()));
}
