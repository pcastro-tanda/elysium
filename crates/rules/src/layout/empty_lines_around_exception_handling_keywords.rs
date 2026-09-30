//! `Layout/EmptyLinesAroundExceptionHandlingKeywords`, ported from
//! RuboCop's
//! `lib/rubocop/cop/layout/empty_lines_around_exception_handling_keywords.rb`.
//! This cop doesn't include the `EmptyLinesAroundBody` mixin the other four
//! `EmptyLinesAround*Body` cops share (see `empty_lines_around_body.rs`):
//! it checks the blank line directly *around* a `rescue`/`else`/`ensure`
//! keyword line itself, not around a construct's own beginning/end.
//!
//! Whitequark nests a `def`/`kwbegin`/`block`'s rescue-and-or-ensure body as
//! `:rescue`/`:ensure` node types, with `:ensure` wrapping a nested `:rescue`
//! (or plain `:begin`) as its first child when both are present. Prism
//! flattens this into one [`NodeKind::BeginNode`] carrying optional
//! `rescue_clause`/`else_clause`/`ensure_clause` fields side by side; a `def`
//! body with `rescue`/`ensure` is this same node kind, implicit (no
//! `begin_keyword_loc`), reached via `DefNode::body`/`BlockNode::body`
//! instead of a `begin_keyword`-bearing top-level node. This port therefore
//! subscribes to [`NodeKind::DefNode`] (covering whitequark's `def`/`defs`
//! alike), [`NodeKind::CallNode`]/[`NodeKind::SuperNode`]/
//! [`NodeKind::ForwardingSuperNode`] (any of which may own a literal block
//! via a `block` field -- whitequark's `on_block`/`on_numblock`/`on_itblock`
//! aliases collapse onto Prism's one [`ruby_ast::node::BlockNode`] kind), and
//! a real `begin ... end` keyword construct's own [`NodeKind::BeginNode`]
//! (guarded by a present `begin_keyword_loc`, so the very same node visited
//! again as a `def`/block's *implicit* body is never double-processed).
//!
//! Each rescue clause's own "rescue" keyword location, an optional trailing
//! `else` clause's keyword, and an optional `ensure` clause's keyword are
//! each independently checked for a blank line immediately above and below.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BeginNode, RescueNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Keeps track of empty lines around exception handling keywords.
#[derive(Debug, Clone)]
pub struct EmptyLinesAroundExceptionHandlingKeywords;

impl Rule for EmptyLinesAroundExceptionHandlingKeywords {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLinesAroundExceptionHandlingKeywords",
        department: Department::Layout,
        summary: "Keeps track of empty lines around exception handling keywords.",
        explanation: "\
```ruby
# good

begin
  do_something
rescue
  do_something2
else
  do_something3
ensure
  do_something4
end

# good

def foo
  do_something
rescue
  do_something2
end

# bad

begin
  do_something

rescue

  do_something2

else

  do_something3

ensure

  do_something4
end

# bad

def foo
  do_something

rescue

  do_something2
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::DefNode,
            NodeKind::CallNode,
            NodeKind::SuperNode,
            NodeKind::ForwardingSuperNode,
            NodeKind::BeginNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::DefNode { .. } => {
                let def = node.as_def_node().expect("kind matched");
                // An endless method (`def foo = expr`) has no `end`
                // keyword and can't carry a `rescue`/`ensure` clause this
                // cop recognises.
                let Some(end_loc) = def.end_keyword_loc() else { return };
                let end_line = ctx.line_col(end_loc.span().start).line;
                let first_line = ctx.line_col(node.span().start).line;
                check_body(ctx, def.body(), first_line, end_line);
            }
            Node::CallNode { .. } | Node::SuperNode { .. } | Node::ForwardingSuperNode { .. } => {
                let block = match node {
                    Node::CallNode { .. } => node.as_call_node().expect("kind matched").block(),
                    Node::SuperNode { .. } => node.as_super_node().expect("kind matched").block(),
                    Node::ForwardingSuperNode { .. } => node
                        .as_forwarding_super_node()
                        .expect("kind matched")
                        .block()
                        .map(|b| b.as_node()),
                    _ => unreachable!("matched above"),
                };
                let Some(block) = block.and_then(|b| b.as_block_node()) else { return };
                let end_line = ctx.line_col(block.closing_loc().span().start).line;
                let first_line = ctx.line_col(node.span().start).line;
                check_body(ctx, block.body(), first_line, end_line);
            }
            Node::BeginNode { .. } => {
                let begin = node.as_begin_node().expect("kind matched");
                // Prism uses the same `BeginNode` for an implicit
                // `rescue`/`ensure` wrapper around a `def`/block body; only
                // a real `begin ... end` keyword construct has its own
                // `begin_keyword_loc` -- the implicit case is already
                // handled above, from its owning `def`/block.
                let (Some(_), Some(end_loc)) = (begin.begin_keyword_loc(), begin.end_keyword_loc())
                else {
                    return;
                };
                let end_line = ctx.line_col(end_loc.span().start).line;
                let first_line = ctx.line_col(node.span().start).line;
                check_body(ctx, Some(*node), first_line, end_line);
            }
            _ => {}
        }
    }
}

/// RuboCop's `check_body`.
fn check_body(ctx: &mut Context<'_>, body: Option<Node<'_>>, first_line: u32, end_line: u32) {
    let Some(body) = body else { return };
    let Some(begin) = body.as_begin_node() else { return };
    if last_body_and_end_on_same_line(ctx, &begin, end_line) {
        return;
    }
    for (line, keyword) in keyword_locations(ctx, &begin) {
        if line == first_line {
            continue;
        }
        report_if_blank(ctx, line + 1, "after", keyword);
        report_if_blank(ctx, line - 1, "before", keyword);
    }
}

/// RuboCop's `keyword_locations`/`keyword_locations_in_rescue`/
/// `keyword_locations_in_ensure`, flattened: Prism's `BeginNode` already
/// carries every clause as a sibling field rather than nesting `:ensure` >
/// `:rescue` the way whitequark does.
fn keyword_locations(ctx: &Context<'_>, begin: &BeginNode<'_>) -> Vec<(u32, &'static str)> {
    let mut locations = Vec::new();
    if let Some(ensure) = begin.ensure_clause() {
        locations.push((ctx.line_col(ensure.ensure_keyword_loc().span().start).line, "ensure"));
    }
    if let Some(resc) = begin.rescue_clause() {
        let mut cur = resc;
        loop {
            locations.push((ctx.line_col(cur.keyword_loc().span().start).line, "rescue"));
            match cur.subsequent() {
                Some(next) => cur = next,
                None => break,
            }
        }
    }
    if let Some(else_clause) = begin.else_clause() {
        locations.push((ctx.line_col(else_clause.else_keyword_loc().span().start).line, "else"));
    }
    locations
}

/// RuboCop's `last_body_and_end_on_same_line?`.
fn last_body_and_end_on_same_line(ctx: &Context<'_>, begin: &BeginNode<'_>, end_line: u32) -> bool {
    if let Some(ensure) = begin.ensure_clause() {
        // `EnsureNode::location` spans through its own `end_keyword_loc`
        // field, i.e. the terminating `end` itself -- unlike whitequark's
        // `:ensure` node, whose source range stops at its last statement
        // (`end` belongs to the wrapping `def`/`kwbegin`/block instead).
        // The last *content* line is therefore the ensure clause's own
        // trailing statements, or (an empty `ensure` clause) the `ensure`
        // keyword's own line.
        let last_line = ensure.statements().map_or_else(
            || ctx.line_col(ensure.ensure_keyword_loc().span().start).line,
            |stmts| ctx.last_line(stmts.as_node().span()),
        );
        return last_line == end_line;
    }
    if let Some(resc) = begin.rescue_clause() {
        let last_line = if let Some(else_clause) = begin.else_clause() {
            ctx.line_col(else_clause.else_keyword_loc().span().start).line
        } else {
            last_resbody_keyword_line(ctx, resc)
        };
        return last_line == end_line;
    }
    false
}

/// The last `rescue` keyword's own line in a chain of `rescue`/`rescue`
/// clauses (whitequark's `resbody_branches.last.loc.keyword.line`).
fn last_resbody_keyword_line(ctx: &Context<'_>, first: RescueNode<'_>) -> u32 {
    let mut cur = first;
    while let Some(next) = cur.subsequent() {
        cur = next;
    }
    ctx.line_col(cur.keyword_loc().span().start).line
}

/// Reports and fixes a blank `line` immediately `location` ("before"/
/// "after") a `keyword`'s own line, if it is in fact blank.
fn report_if_blank(ctx: &mut Context<'_>, line: u32, location: &str, keyword: &str) {
    if ctx.line_text(line).is_empty() {
        let start = ctx.line_span(line).start;
        let span = Span::new(start, start + 1);
        let msg = format!("Extra empty line detected {location} the `{keyword}`.");
        let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] };
        ctx.report_with_fix(&EmptyLinesAroundExceptionHandlingKeywords::META, span, msg, fix);
    }
}
