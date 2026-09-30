//! `Layout/EmptyLineAfterGuardClause`, ported from RuboCop's
//! `lib/rubocop/cop/layout/empty_line_after_guard_clause.rb`.
//!
//! Upstream's `correct_style?` decides whether a guard-clause `if`/`unless`
//! needs a following blank line by walking whitequark's real parent/sibling
//! links (`node.parent`, `node.right_sibling`). Prism has no `begin`-elision:
//! every statement body is a [`NodeKind::StatementsNode`] even when it holds
//! exactly one item, so there is no direct Prism analogue of "this node's
//! whitequark parent is the enclosing `if`/`rescue`/`ensure` node itself".
//!
//! Working out RuboCop's four `correct_style?`/`multiple_statements_on_line?`
//! disjuncts by hand shows every one of them reduces to "is the node's real
//! *next sibling statement* (in the plain sense: the next item in the same
//! [`NodeKind::StatementsNode`]) either absent, on the same source line, or
//! itself a guard-clause `if`/`unless`?" -- the whitequark quirks (a
//! single-statement `if`/`unless` branch's "parent" being the conditional
//! itself and reaching its `elsif`/`else`; a `rescue`/`ensure`-wrapped main
//! body's "parent" being the `rescue`/`ensure` node) *always* resolve to
//! "skip" on their own, and a node with no plain next sibling in its own
//! [`NodeKind::StatementsNode`] (the sole statement of a `def`/block/class
//! body, a `rescue` clause, an `ensure` clause, or the whole program) never
//! reaches any of those quirks anyway. So this port tracks only the plain
//! sibling relationship: [`EmptyLineAfterGuardClause::siblings`] is
//! precomputed once per [`NodeKind::StatementsNode`] (every item but the
//! last), and any node absent from it -- the last item of some body, or not
//! a body item at all -- defaults to "no plain sibling", which always means
//! skip.
//!
//! A modifier-form guard clause's heredoc argument is deferred exactly like
//! upstream: the guard node's own source range stops at the heredoc's
//! opening tag, so the blank-line check and its correction must instead
//! anchor on the heredoc's closing delimiter line ([`heredoc_argument`]/
//! [`heredoc_closing_line_and_span`]), mirroring `last_heredoc_argument`/
//! `heredoc_line` -- simplified to look at the closing delimiter's own
//! physical line directly rather than replicating the original's
//! `heredoc_body.last_line - heredoc_body.first_line` arithmetic (both
//! compute the same target line; Prism's `closing_loc` makes the direct
//! route available).

use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::is_heredoc;
use ruby_ast::node::StatementsNode;
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeKind};
use ruby_directives::DirectiveKind;
use ruby_source::Span;

/// Add empty line after guard clause.
#[derive(Debug, Clone, Default)]
pub struct EmptyLineAfterGuardClause {
    /// For every node that is not the last item of the [`NodeKind::StatementsNode`]
    /// containing it, keyed by that node's own span start: whether its plain
    /// next sibling already satisfies `correct_style?`/
    /// `multiple_statements_on_line?` (same line, or itself a guard-clause
    /// `if`/`unless`). Absent from the map (the last item of some body, or
    /// not a body item at all) always means "no plain sibling", which is
    /// also always a skip -- see the module doc comment.
    siblings: HashMap<Span, bool>,
}

impl Rule for EmptyLineAfterGuardClause {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLineAfterGuardClause",
        department: Department::Layout,
        summary: "Add empty line after guard clause.",
        explanation: "\
Checks for guard clauses (`return`/`break`/`next`/`raise`/`fail`, possibly
`and`/`or`-wrapped) not followed by an empty line.

```ruby
# bad
def foo
  return if need_return?
  bar
end

# good
def foo
  return if need_return?

  bar
end

# good
def foo
  return if something?
  return if something_different?

  bar
end

# also good
def foo
  if something?
    do_something
    return if need_return?
  end
end
```

A SimpleCov directive comment (`# :nocov:` or `# simplecov:disable`/
`# simplecov:enable`) directly after the guard clause is allowed, since
SimpleCov excludes code from the coverage report by wrapping it in such
directives.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode, NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[],
        blind_spots: "\
`node.parent&.assignment?`-style whitequark-parent quirks (a single-statement
`if`/`unless` branch reaching its own `elsif`/`else`; a `rescue`/`ensure`
main body reaching the `rescue`/`ensure` node) are not walked explicitly --
see the module doc comment for why the plain next-sibling check this port
uses instead always agrees with them.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::StatementsNode { .. } => {
                let s = node.as_statements_node().expect("kind matched");
                register_statements(&mut self.siblings, ctx, s);
            }
            Node::IfNode { .. } | Node::UnlessNode { .. } => {
                check_guard_clause(&self.siblings, node, ctx);
            }
            _ => {}
        }
    }
}

/// A normalized `if`/`unless` branch body, mirroring `rubocop-ast`'s
/// `IfNode#if_branch`: a single statement is unwrapped, an empty body is
/// `Empty`, and multiple statements stay as the underlying `StatementsNode`
/// (RuboCop's synthetic `begin`).
enum Branch<'pr> {
    Empty,
    Single(Node<'pr>),
    Multi,
}

fn branch_of(stmts: Option<StatementsNode<'_>>) -> Branch<'_> {
    let Some(stmts) = stmts else { return Branch::Empty };
    let items = stmts.body();
    match items.len() {
        0 => Branch::Empty,
        1 => Branch::Single(items.first().expect("len == 1")),
        _ => Branch::Multi,
    }
}

/// A normalized view of one `if`/`unless` node, spanning both Prism node
/// kinds RuboCop's whitequark-based AST unifies, and the parts this cop's
/// checks need. A ternary (`a ? b : c`, also an `IfNode` in Prism, with no
/// `if`/`elsif` keyword) is a valid guard-clause site too -- RuboCop's
/// `on_if` fires for it the same as any other `:if` node.
struct Shape<'pr> {
    node_span: Span,
    /// RuboCop's `IfNode#modifier_form?`: true only for a real `if`/`unless`
    /// with no `end` keyword (a ternary has none either, but is never a
    /// modifier).
    is_modifier: bool,
    end_span: Option<Span>,
    if_branch: Branch<'pr>,
    predicate: Node<'pr>,
}

fn shape_of<'pr>(node: &Node<'pr>) -> Option<Shape<'pr>> {
    match node {
        Node::IfNode { .. } => {
            let n = node.as_if_node().expect("kind matched");
            let is_ternary = n.if_keyword_loc().is_none();
            let end_span = n.end_keyword_loc().map(|l| l.span());
            Some(Shape {
                node_span: node.location().span(),
                is_modifier: !is_ternary && end_span.is_none(),
                end_span,
                if_branch: branch_of(n.statements()),
                predicate: n.predicate(),
            })
        }
        Node::UnlessNode { .. } => {
            let n = node.as_unless_node().expect("kind matched");
            let end_span = n.end_keyword_loc().map(|l| l.span());
            Some(Shape {
                node_span: node.location().span(),
                is_modifier: end_span.is_none(),
                end_span,
                if_branch: branch_of(n.statements()),
                predicate: n.predicate(),
            })
        }
        _ => None,
    }
}

/// RuboCop's `match_guard_clause?`'s base pattern (before the `single_line?`
/// requirement): a bare `raise`/`fail` call, or a `return`/`break`/`next`.
fn is_guard_exit(n: &Node<'_>) -> bool {
    match n {
        Node::ReturnNode { .. } | Node::BreakNode { .. } | Node::NextNode { .. } => true,
        Node::CallNode { .. } => {
            let c = n.as_call_node().expect("kind matched");
            c.receiver().is_none() && matches!(c.name().as_slice(), b"raise" | b"fail")
        }
        _ => false,
    }
}

/// The right-hand side of an `and`/`or`, when `n` is one.
fn and_or_rhs<'pr>(n: &Node<'pr>) -> Option<Node<'pr>> {
    match n {
        Node::AndNode { .. } => Some(n.as_and_node().expect("kind matched").right()),
        Node::OrNode { .. } => Some(n.as_or_node().expect("kind matched").right()),
        _ => None,
    }
}

/// `rubocop-ast`'s generic `Node#guard_clause?`: unwraps a trailing
/// `and`/`or` (`Node#operator_keyword?`/`#rhs`), then requires the result to
/// both be a guard-exit shape and single-line (`match_guard_clause?`'s
/// `single_line?` conjunct). Used for the guard-if/unless's *own*
/// `if_branch` -- the only place upstream's `correct_style?` checks this.
///
/// A heredoc-bodied guard exit (`raise <<-MSG unless x`) still passes: a
/// Prism heredoc node's own span (like whitequark's) stops at the opening
/// tag, so the enclosing guard-exit node is single-line even though its
/// rendered heredoc body spans further lines.
fn branch_is_guard_clause_generic(branch: &Branch<'_>, ctx: &Context<'_>) -> bool {
    let Branch::Single(n) = branch else { return false };
    let target = and_or_rhs(n).unwrap_or(*n);
    is_guard_exit(&target) && ctx.is_single_line(target.location().span())
}

/// RuboCop's `contains_guard_clause?`: `branch.guard_clause?` (the generic,
/// single-line-required check above) OR'd with the cop's own
/// `guard_clause_branch?` matcher -- a *direct* (non-`and`/`or`-unwrapped)
/// guard-exit match with no single-line requirement, so a genuinely
/// multi-line `raise`/`return` still counts here. Used only to decide
/// whether a *sibling* if/unless is itself a guard clause.
fn branch_is_guard_clause_any(branch: &Branch<'_>, ctx: &Context<'_>) -> bool {
    let Branch::Single(n) = branch else { return false };
    is_guard_exit(n) || branch_is_guard_clause_generic(branch, ctx)
}

/// RuboCop's `contains_guard_clause?(node)`: `node` is an `if`/`unless`
/// (including a ternary) whose own `if_branch` is a guard clause.
fn node_contains_guard_clause(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let stmts = match node {
        Node::IfNode { .. } => node.as_if_node().expect("kind matched").statements(),
        Node::UnlessNode { .. } => node.as_unless_node().expect("kind matched").statements(),
        _ => return false,
    };
    branch_is_guard_clause_any(&branch_of(stmts), ctx)
}

/// Precomputes [`EmptyLineAfterGuardClause::siblings`] for every item but
/// the last in one `StatementsNode`'s body: whether that item's plain next
/// sibling already satisfies `correct_style?`/`multiple_statements_on_line?`
/// (same source line, or itself a guard-clause `if`/`unless`).
fn register_statements(map: &mut HashMap<Span, bool>, ctx: &Context<'_>, s: StatementsNode<'_>) {
    let items = s.body();
    let len = items.len();
    if len < 2 {
        return;
    }
    let mut iter = items.iter();
    let mut cur = iter.next().expect("len >= 2");
    for _ in 1..len {
        let next = iter.next().expect("iterator matches len");
        let cur_span = cur.location().span();
        let next_span = next.location().span();
        let same_line = ctx.line_col(cur_span.start).line == ctx.line_col(next_span.start).line;
        let skip = same_line || node_contains_guard_clause(&next, ctx);
        map.insert(cur_span, skip);
        cur = next;
    }
}

/// Unwraps a parenthesized single-statement grouping (Prism's
/// `ParenthesesNode` wrapping a one-item `StatementsNode`), mirroring
/// RuboCop's `n.children.first while n.respond_to?(:begin_type?) &&
/// n.begin_type?` (whitequark represents `(expr)` as a `begin` node even for
/// one statement).
fn unwrap_parens(mut n: Node<'_>) -> Node<'_> {
    while let Node::ParenthesesNode { .. } = n {
        let Some(body) = n.as_parentheses_node().expect("kind matched").body() else { break };
        let Node::StatementsNode { .. } = body else { break };
        let items = body.as_statements_node().expect("kind matched").body();
        if items.len() != 1 {
            break;
        }
        n = items.first().expect("len == 1");
    }
    n
}

/// RuboCop's `node.if_branch.children.last`, restricted to the guard-exit
/// shapes it is ever called on: a bare `raise`/`fail` call's last argument,
/// or a `return`/`break`/`next`'s last value.
fn last_child_of<'pr>(n: &Node<'pr>) -> Option<Node<'pr>> {
    match n {
        Node::CallNode { .. } => {
            n.as_call_node().expect("kind matched").arguments()?.arguments().last()
        }
        Node::ReturnNode { .. } => {
            n.as_return_node().expect("kind matched").arguments()?.arguments().last()
        }
        Node::BreakNode { .. } => {
            n.as_break_node().expect("kind matched").arguments()?.arguments().last()
        }
        Node::NextNode { .. } => {
            n.as_next_node().expect("kind matched").arguments()?.arguments().last()
        }
        _ => None,
    }
}

/// RuboCop's `use_heredoc_in_condition?`: does any descendant of `node`
/// carry a heredoc-capable literal?
fn heredoc_in_subtree(node: &Node<'_>) -> bool {
    let mut found = false;
    each_descendant(node, &mut |n| {
        if is_heredoc(n) {
            found = true;
        }
    });
    found
}

/// RuboCop's `last_heredoc_argument_node`: picks which part of the guard-if
/// to start the heredoc search from -- the left side of an `and`-wrapped
/// `if_branch`, the predicate itself when it carries a heredoc, or otherwise
/// the `if_branch`'s own last child.
fn last_heredoc_argument_node<'pr>(shape: &Shape<'pr>) -> Option<Node<'pr>> {
    if let Branch::Single(n) = &shape.if_branch {
        if let Node::AndNode { .. } = n {
            return Some(n.as_and_node().expect("kind matched").left());
        }
    }
    if heredoc_in_subtree(&shape.predicate) {
        return Some(shape.predicate);
    }
    match &shape.if_branch {
        Branch::Single(n) => last_child_of(n),
        _ => None,
    }
}

/// RuboCop's `last_heredoc_argument`: descends through a parenthesized
/// grouping, then a call's arguments and finally its receiver, looking for
/// a heredoc-capable literal.
fn last_heredoc_argument(node: Node<'_>) -> Option<Node<'_>> {
    let n = unwrap_parens(node);
    if is_heredoc(&n) {
        return Some(n);
    }
    let Node::CallNode { .. } = &n else { return None };
    let c = n.as_call_node().expect("kind matched");
    if let Some(args) = c.arguments() {
        for arg in &args.arguments() {
            if let Some(found) = last_heredoc_argument(arg) {
                return Some(found);
            }
        }
    }
    c.receiver().and_then(last_heredoc_argument)
}

/// The heredoc argument a modifier-form guard clause defers, if any.
fn heredoc_argument<'pr>(shape: &Shape<'pr>) -> Option<Node<'pr>> {
    last_heredoc_argument(last_heredoc_argument_node(shape)?)
}

/// The closing-delimiter span of a heredoc-capable string literal.
fn heredoc_closing_span(n: &Node<'_>) -> Option<Span> {
    match n {
        Node::StringNode { .. } => {
            n.as_string_node().expect("kind matched").closing_loc().map(|l| l.span())
        }
        Node::InterpolatedStringNode { .. } => {
            n.as_interpolated_string_node().expect("kind matched").closing_loc().map(|l| l.span())
        }
        Node::XStringNode { .. } => {
            Some(n.as_x_string_node().expect("kind matched").closing_loc().span())
        }
        Node::InterpolatedXStringNode { .. } => {
            Some(n.as_interpolated_x_string_node().expect("kind matched").closing_loc().span())
        }
        _ => None,
    }
}

/// The heredoc's closing delimiter's own 1-based line, and its span trimmed
/// of the trailing line terminator Prism's `closing_loc` includes (unlike
/// whitequark's `loc.heredoc_end`) -- both the offense location and the
/// `next_line_empty_or_allowed_directive_comment?`/autocorrect anchor.
fn heredoc_closing_line_and_span(ctx: &Context<'_>, n: &Node<'_>) -> Option<(u32, Span)> {
    let span = heredoc_closing_span(n)?;
    let text = ctx.text(span);
    let trimmed_len =
        text.len() - text.iter().rev().take_while(|&&b| b == b'\n' || b == b'\r').count();
    let end = span.start + u32::try_from(trimmed_len).unwrap_or(span.end - span.start);
    let line = ctx.line_col(span.start).line;
    Some((line, Span::new(span.start, end)))
}

/// `String#blank?` on a 1-based physical line; a line past the end of the
/// file counts as blank, matching `processed_source[line]` being `nil`
/// (`nil.blank?` is true).
fn is_blank_line(ctx: &Context<'_>, line: u32) -> bool {
    if line > ctx.line_count() {
        return true;
    }
    ctx.line_text(line).iter().all(u8::is_ascii_whitespace)
}

/// RuboCop's `SIMPLECOV_COMMENT_PATTERN`:
/// `/\A#\s*(?::nocov:|simplecov\s*:\s*(?:disable|enable)\b)/`.
fn is_simplecov_comment(text: &[u8]) -> bool {
    let mut i = match text.first() {
        Some(b'#') => 1,
        _ => return false,
    };
    while text.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    if text[i..].starts_with(b":nocov:") {
        return true;
    }
    let Some(rest) = text[i..].strip_prefix(b"simplecov") else { return false };
    let mut j = 0;
    while rest.get(j).is_some_and(u8::is_ascii_whitespace) {
        j += 1;
    }
    if rest.get(j) != Some(&b':') {
        return false;
    }
    j += 1;
    while rest.get(j).is_some_and(u8::is_ascii_whitespace) {
        j += 1;
    }
    let tail = &rest[j..];
    [&b"disable"[..], &b"enable"[..]].into_iter().any(|kw| {
        tail.starts_with(kw)
            && tail.get(kw.len()).is_none_or(|b| !(b.is_ascii_alphanumeric() || *b == b'_'))
    })
}

/// RuboCop's `next_line_allowed_directive_comment?`: a `# rubocop:enable`
/// (or `enable-next`) directive, or a `SimpleCov` directive comment, on the
/// given 1-based physical line.
fn allowed_directive_line(ctx: &Context<'_>, line: u32) -> bool {
    let has_enable = ctx.directives().directives().iter().any(|d| {
        d.line == line && matches!(d.kind, DirectiveKind::Enable | DirectiveKind::EnableNext)
    });
    if has_enable {
        return true;
    }
    ctx.comments().iter().any(|c| c.line == line && is_simplecov_comment(ctx.text(c.span)))
}

/// RuboCop's `next_line_empty_or_allowed_directive_comment?`: the physical
/// line right after `anchor_last_line` is blank, or it is an allowed
/// directive/SimpleCov comment and the line after *that* is blank.
fn next_line_ok(ctx: &Context<'_>, anchor_last_line: u32) -> bool {
    let next = anchor_last_line + 1;
    if is_blank_line(ctx, next) {
        return true;
    }
    allowed_directive_line(ctx, next) && is_blank_line(ctx, next + 1)
}

/// RuboCop's `on_if`, `correct_style?`, `multiple_statements_on_line?` and
/// `autocorrect` combined: reports and fixes a guard clause not followed by
/// a blank line.
fn check_guard_clause(siblings: &HashMap<Span, bool>, node: &Node<'_>, ctx: &mut Context<'_>) {
    let Some(shape) = shape_of(node) else { return };
    if !branch_is_guard_clause_generic(&shape.if_branch, ctx) {
        return;
    }
    if *siblings.get(&shape.node_span).unwrap_or(&true) {
        return;
    }

    let heredoc_closing = if shape.is_modifier {
        heredoc_argument(&shape).and_then(|h| heredoc_closing_line_and_span(ctx, &h))
    } else {
        None
    };

    let (anchor_line, offense_span) = match heredoc_closing {
        Some((line, span)) => (line, span),
        None => (ctx.last_line(shape.node_span), shape.end_span.unwrap_or(shape.node_span)),
    };

    if next_line_ok(ctx, anchor_line) {
        return;
    }

    let insert_line =
        if allowed_directive_line(ctx, anchor_line + 1) { anchor_line + 1 } else { anchor_line };
    let insert_at = ctx.line_span(insert_line).end;
    let fix = Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::insert(insert_at, b"\n".as_slice())],
    };
    ctx.report_with_fix(
        &EmptyLineAfterGuardClause::META,
        offense_span,
        "Add empty line after guard clause.",
        fix,
    );
}
