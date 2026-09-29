//! `Style/IdenticalConditionalBranches`, ported from RuboCop's
//! `lib/rubocop/cop/style/identical_conditional_branches.rb`.
//!
//! # Branch shapes
//!
//! Whitequark's `on_if` fires for both `if` and `unless` (the parser
//! reuses the single `:if` node type for `unless`, only swapping branch
//! order); Prism splits them into distinct `IfNode`/`UnlessNode` kinds, so
//! both are subscribed here. `elsif` clauses show up as a nested `:if` in
//! whitequark's `else_branch`; Prism represents the same chain as nested
//! `IfNode`s off `subsequent()`, distinguished from a real `else` (an
//! `ElseNode`) by node kind, and from a fresh `if` by its `if_keyword_loc`
//! reading `"elsif"` rather than `"if"`. A ternary is an `IfNode` with no
//! `if_keyword_loc` at all (the `then_keyword_loc` field is reused for the
//! `?` token instead).
//!
//! Every branch body is a Prism `StatementsNode`; whitequark elides the
//! `begin` wrapper for a single statement, so a single-statement branch's
//! head and tail are the same node, and a multi-statement branch's head/
//! tail are its first/last element. `()` is whitequark's `begin` node with
//! zero children (Prism: a `ParenthesesNode` with no `body`); it reduces to
//! no expression at all rather than crashing, matching upstream's
//! `does_not_raise_any_error_when_using_empty_parentheses` specs.
//!
//! Upstream's own branch-gathering (`expand_elses`) has an incidental bug
//! that is faithfully reproduced rather than fixed: since whitequark's `:if`
//! node type covers a real `if`/`elsif` chain *and* every `unless` alike, a
//! branch whose sole (elided) statement happens to be an unrelated modifier
//! or block `if`/`unless` gets peeled apart as though *it* were an `elsif`
//! of the outer conditional -- see [`expand_else_position`]'s doc for a
//! worked, corpus-confirmed example.
//!
//! # The two soundness guards
//!
//! Upstream's `duplicated_expressions?` does double duty: besides checking
//! that every branch's head (or tail) is the same expression, it also
//! blocks hoisting an assignment whose condition reads the same
//! target -- reordering `x = do_something` ahead of `if x.condition`
//! would change what the condition sees. rubocop-ast's `Node#assignment?`
//! is overridden by `MethodDispatchNode` (`alias assignment? setter_method?`)
//! for any `send`/`csend` carrying an `=` location, so `h[:key] = foo`
//! (parsed as a plain `send` to `:[]=`, not a dedicated `indexasgn` node)
//! counts too. A second, narrower guard in `check_branches` additionally
//! skips hoisting a *head* assignment before the condition when the
//! condition and the assignment target read the same plain variable/
//! receiver text; upstream computes this via `node_parts[0].to_s` for
//! every other assignment shape (shorthand `op_asgn`/`or_asgn`/`and_asgn`,
//! `casgn`, `masgn`, a `csend` setter), which yields a `Node#inspect`-style
//! s-expression that can never equal a plain identifier, making that
//! branch of the guard permanently inert; this port reproduces the
//! *effect* (never firing) by simply not computing a value for those
//! shapes instead of replicating the always-false string.
//!
//! # Comparing expressions
//!
//! Two branches' head/tail expressions are compared by source text, plus
//! (recursively, for every heredoc anywhere in either subtree) their body:
//! a heredoc's own `location` -- and every ancestor's around it, up
//! through the enclosing call/assignment -- covers only the opening
//! `<<~TAG` declaration, since Prism finalises those spans before the
//! lazily-attached, later-positioned body is parsed. Comparing by source
//! text alone would make two textually-identical `<<~TAG` openings with
//! different bodies compare equal (confirmed false positives against real
//! RuboCop 1.91.0 in the discourse and mastodon corpora, e.g.
//! `application_helper.rb`'s two distinct `result << <<~HTML` calls).
//!
//! # Autocorrection
//!
//! Autocorrection reorders code around the conditional and is therefore
//! unsafe (`@safety` note upstream). It is additionally skipped entirely
//! for a ternary or an explicit `then` (`node.if_type? && (node.ternary? ||
//! node.then?)`), both of which have nowhere on their own line to place a
//! hoisted statement.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, NodeInfo, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::is_heredoc;
use ruby_ast::node::{CallNode, IfNode, StatementsNode, UnlessNode};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
/// Which side of the conditional a hoisted expression lands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InsertPosition {
    /// The expression was the last statement of every branch: moves after
    /// the conditional.
    AfterCondition,
    /// The expression was the first statement of every branch: moves
    /// before the conditional.
    BeforeCondition,
}

/// Checks that conditional statements do not have an identical expression
/// at the start or end of every branch, which can be moved outside the
/// conditional instead.
///
/// # Examples
///
/// ```ruby
/// # bad
/// if condition
///   do_x
///   do_z
/// else
///   do_y
///   do_z
/// end
///
/// # good
/// if condition
///   do_x
/// else
///   do_y
/// end
/// do_z
///
/// # bad
/// case foo
/// when 1
///   do_x
/// when 2
///   do_x
/// else
///   do_x
/// end
///
/// # good
/// case foo
/// when 1
///   do_x
///   do_y
/// when 2
///   # nothing
/// else
///   do_x
///   do_z
/// end
/// ```
#[derive(Debug, Clone)]
pub struct IdenticalConditionalBranches {
    /// For each currently-open `StatementsNode`, the span of its last
    /// element (or `None` if it has none) -- lets [`Self::is_last_child`]
    /// answer "is this conditional the final statement of its immediate
    /// container" without needing the parent's own accessors, which the
    /// engine's flat ancestor stack does not expose.
    stmt_last_stack: Vec<Option<Span>>,
}

impl Rule for IdenticalConditionalBranches {
    const META: RuleMeta = RuleMeta {
        name: "Style/IdenticalConditionalBranches",
        department: Department::Style,
        summary: "Checks that conditional statements do not have an identical line at the end of each branch, which can validly be moved out of the conditional.",
        explanation: "\
Checks for identical expressions at the beginning or end of each branch of
a conditional expression. Such expressions should normally be placed
outside the conditional expression - before or after it.

```ruby
# bad
if condition
  do_x
  do_z
else
  do_y
  do_z
end

# good
if condition
  do_x
else
  do_y
end
do_z

# bad
if condition
  do_z
  do_x
else
  do_z
  do_y
end

# good
do_z
if condition
  do_x
else
  do_y
end

# bad
case foo
when 1
  do_x
when 2
  do_x
else
  do_x
end

# good
case foo
when 1
  do_x
  do_y
when 2
  # nothing
else
  do_x
  do_z
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::StatementsNode,
            NodeKind::IfNode,
            NodeKind::UnlessNode,
            NodeKind::CaseNode,
            NodeKind::CaseMatchNode,
        ],
        config: &[],
        blind_spots: "\
Autocorrection is unsafe: it can reorder method invocations across a
condition that depends on global state. The condition-matches-target guard
that blocks hoisting `x = do_something` above `if x.condition` (or a
`send`/`csend` setter target) is ported faithfully; the equivalent guard
for every other assignment shape (shorthand `op_asgn`/`or_asgn`/`and_asgn`,
`casgn`, `masgn`, a `csend` setter target) is upstream-inert (it compares a
plain identifier against a `Node#inspect`-style string that can never
match) and is reproduced here by simply never matching those shapes,
rather than replicating the unreachable comparison. `last_child_of_parent?`
is exact when the conditional's parent is a `StatementsNode` (its usual
position) or has no parent; for a non-`StatementsNode`, non-nil parent
(e.g. a hash value, an array element, a call argument) it defaults to
`true`, matching every parent shape exercised by the corpus but not
verified against upstream's own `child_nodes.last == node` check for the
untested shapes (e.g. the conditional as a non-last call argument).",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { stmt_last_stack: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.stmt_last_stack.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::StatementsNode => {
                let stmts = node.as_statements_node().expect("kind matched");
                self.stmt_last_stack.push(stmts.body().last().map(|n| n.span()));
            }
            NodeKind::IfNode => self.handle_if(node, ctx),
            NodeKind::UnlessNode => self.handle_unless(node, ctx),
            NodeKind::CaseNode => self.handle_case(node, ctx),
            NodeKind::CaseMatchNode => self.handle_case_match(node, ctx),
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.kind() == NodeKind::StatementsNode {
            self.stmt_last_stack.pop();
        }
    }
}

impl IdenticalConditionalBranches {
    /// `on_if`, minus the `elsif?` early return (an `elsif` chain is
    /// consumed whole by its owning `if`, via [`if_branches`]).
    fn handle_if(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let if_node = node.as_if_node().expect("kind matched");
        if is_elsif(&if_node, ctx) {
            return;
        }
        let Some(branches) = if_branches(&if_node) else { return };
        let condition = if_node.predicate();
        let is_ternary = if_node.if_keyword_loc().is_none();
        let has_then =
            if_node.then_keyword_loc().is_some_and(|loc| ctx.text(loc.span()) == b"then");
        self.check_branches(ctx, node.span(), Some(&condition), &branches, is_ternary || has_then);
    }

    /// `on_if`'s `unless` half: whitequark reuses the same `:if` node type
    /// (with its two branches swapped) for `unless`, so upstream's single
    /// callback covers it; Prism gives `unless` its own kind instead.
    fn handle_unless(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let unless_node = node.as_unless_node().expect("kind matched");
        let Some(branches) = unless_branches(&unless_node) else { return };
        let condition = unless_node.predicate();
        let has_then =
            unless_node.then_keyword_loc().is_some_and(|loc| ctx.text(loc.span()) == b"then");
        self.check_branches(ctx, node.span(), Some(&condition), &branches, has_then);
    }

    fn handle_case(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let case_node = node.as_case_node().expect("kind matched");
        let Some(else_clause) = case_node.else_clause() else { return };
        let mut branches: Vec<Option<Node<'_>>> = Vec::new();
        for clause in &case_node.conditions() {
            let Some(when) = clause.as_when_node() else { return };
            branches.push(elide(when.statements()));
        }
        branches.push(elide(else_clause.statements()));
        let Some(branches) = branches.into_iter().collect::<Option<Vec<_>>>() else { return };
        let condition = case_node.predicate();
        self.check_branches(ctx, node.span(), condition.as_ref(), &branches, false);
    }

    fn handle_case_match(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let case_node = node.as_case_match_node().expect("kind matched");
        let Some(else_clause) = case_node.else_clause() else { return };
        let mut branches: Vec<Option<Node<'_>>> = Vec::new();
        for clause in &case_node.conditions() {
            let Some(in_node) = clause.as_in_node() else { return };
            branches.push(elide(in_node.statements()));
        }
        branches.push(elide(else_clause.statements()));
        let Some(branches) = branches.into_iter().collect::<Option<Vec<_>>>() else { return };
        let condition = case_node.predicate();
        self.check_branches(ctx, node.span(), condition.as_ref(), &branches, false);
    }

    /// `check_branches`.
    fn check_branches(
        &self,
        ctx: &mut Context<'_>,
        node_span: Span,
        condition: Option<&Node<'_>>,
        branches: &[Node<'_>],
        skip_fix: bool,
    ) {
        let tails: Vec<Option<Node<'_>>> = branches.iter().map(branch_tail).collect();
        if duplicated_expressions(ctx, condition, &tails) {
            Self::check_expressions(
                ctx,
                node_span,
                &tails,
                InsertPosition::AfterCondition,
                skip_fix,
            );
        }

        if self.is_last_child(ctx, node_span) && branches.iter().any(single_child_branch) {
            return;
        }

        let heads: Vec<Option<Node<'_>>> = branches.iter().map(branch_head).collect();
        if !duplicated_expressions(ctx, condition, &heads) {
            return;
        }

        if let Some(Some(head)) = heads.first() {
            if is_assignment_kind(head) {
                let condition_text =
                    condition.and_then(assignable_condition_value).map(|s| ctx.text(s));
                let assigned_text = assigned_value_span(head).map(|s| ctx.text(s));
                if condition_text.is_some() && condition_text == assigned_text {
                    return;
                }
            }
        }

        Self::check_expressions(ctx, node_span, &heads, InsertPosition::BeforeCondition, skip_fix);
    }

    /// RuboCop's `last_child_of_parent?`: our node has no parent, or is the
    /// last element of a parent `StatementsNode`. See the type's blind
    /// spot note for non-`StatementsNode` parents.
    fn is_last_child(&self, ctx: &Context<'_>, node_span: Span) -> bool {
        match ctx.parent() {
            Some(NodeInfo { kind: NodeKind::StatementsNode, .. }) => {
                self.stmt_last_stack.last().copied().flatten() == Some(node_span)
            }
            None | Some(_) => true,
        }
    }

    /// `check_expressions`: reports (and, unless `skip_fix`, corrects)
    /// every occurrence of the duplicated expression. Only the first
    /// occurrence's fix carries the hoisted insertion; every occurrence's
    /// fix deletes its own whole line(s).
    fn check_expressions(
        ctx: &mut Context<'_>,
        node_span: Span,
        expressions: &[Option<Node<'_>>],
        position: InsertPosition,
        skip_fix: bool,
    ) {
        if expressions.iter().any(Option::is_none) {
            return;
        }
        let parent = ctx.parent();
        let mut inserted = false;
        for expr in expressions.iter().map(|e| e.as_ref().expect("checked above")) {
            let source = String::from_utf8_lossy(ctx.text(expr.span()));
            let message = format!("Move `{source}` out of the conditional.");
            if skip_fix {
                ctx.report(&Self::META, expr.span(), message);
                continue;
            }
            let mut edits = vec![Edit::delete(ctx.whole_lines(expr.span()))];
            if !inserted {
                edits.extend(insertion_edits(ctx, node_span, parent, expr, position));
                inserted = true;
            }
            ctx.report_with_fix(
                &Self::META,
                expr.span(),
                message,
                Fix { applicability: Applicability::Unsafe, edits },
            );
        }
    }
}

/// RuboCop's `IfNode#elsif?`: this `if` is itself the `subsequent` of an
/// enclosing `if`, spelled `elsif` rather than `if` at the keyword.
fn is_elsif(if_node: &IfNode<'_>, ctx: &Context<'_>) -> bool {
    if_node.if_keyword_loc().is_some_and(|loc| ctx.text(loc.span()) == b"elsif")
}

/// Whitequark's parser-level elision: a `StatementsNode` with exactly one
/// statement reduces to that statement itself (no wrapping `begin` node);
/// two or more statements reduce to the `StatementsNode` itself, standing
/// in for whitequark's `begin`; no statements at all stays `None`.
fn elide(stmts: Option<StatementsNode<'_>>) -> Option<Node<'_>> {
    let stmts = stmts?;
    let body = stmts.body();
    if body.len() == 1 {
        body.first()
    } else {
        Some(stmts.as_node())
    }
}

/// The "else"-position value contributed by an `IfNode`'s own
/// `subsequent()`: a real `elsif` is Prism's own nested `IfNode`, already
/// bare (no `StatementsNode` involved, so no elision needed) and passed
/// through for [`expand_else_position`] to peel further; a real `else`
/// elides its body the same way any other branch does.
fn elide_subsequent(subsequent: Option<Node<'_>>) -> Option<Node<'_>> {
    match subsequent {
        Some(node) if node.as_if_node().is_some() => Some(node),
        Some(node) => node.as_else_node().and_then(|e| elide(e.statements())),
        None => None,
    }
}

/// Whitequark's `expand_elses`, reproduced including its incidental bug:
/// since whitequark's `:if` node type covers a real `if`/`elsif` *and* a
/// modifier/block `unless` alike, a branch whose sole (elided) statement
/// happens to be an unrelated `if`/`unless` gets peeled apart as though
/// *it* were an `elsif` clause of the outer conditional -- discarding one
/// of its own slots as a phantom missing branch, which then makes the
/// whole `branches.any?(&:nil?)` check bail on the *outer* conditional
/// too. Confirmed against real RuboCop 1.91.0 (`app/models/group.rb` in
/// the discourse corpus: an `if`/`else` whose `else` is a single `x = y
/// unless z` statement reports no offense upstream, for exactly this
/// reason) and reproduced faithfully rather than "fixed", since fixing it
/// would diverge from upstream's real, observable behaviour.
fn expand_else_position(branch: Option<Node<'_>>) -> Vec<Option<Node<'_>>> {
    let Some(node) = branch else { return vec![None] };
    let (truthy, falsy) = if let Some(inner) = node.as_if_node() {
        (elide(inner.statements()), elide_subsequent(inner.subsequent()))
    } else if let Some(inner) = node.as_unless_node() {
        // Whitequark swaps `unless`'s two slots relative to `if`: its
        // `else`-clause body is the canonical "truthy" slot, its own body
        // the canonical "falsy" one that keeps recursing.
        (inner.else_clause().and_then(|e| elide(e.statements())), elide(inner.statements()))
    } else {
        return vec![Some(node)];
    };
    let mut rest = expand_else_position(falsy);
    rest.insert(0, truthy);
    rest
}

/// `expand_elses(node.else_branch).unshift(node.if_branch)`: the `if`
/// branch (never peeled, matching upstream), then every `elsif`/coincident
/// branch in order, then the final `else` branch. `None` anywhere (a
/// missing branch, real or the incidental-bug kind) makes the whole `if`
/// unactionable, matching upstream's `branches.any?(&:nil?)` bail.
fn if_branches<'pr>(if_node: &IfNode<'pr>) -> Option<Vec<Node<'pr>>> {
    let truthy = elide(if_node.statements());
    let mut branches = expand_else_position(elide_subsequent(if_node.subsequent()));
    branches.insert(0, truthy);
    branches.into_iter().collect()
}

/// `unless`'s own `on_if` branches, canonicalised (see
/// [`expand_else_position`]'s `UnlessNode` arm) to whitequark's swapped
/// layout before the same incidental-bug peeling is applied to its own
/// ("falsy") body.
fn unless_branches<'pr>(unless_node: &UnlessNode<'pr>) -> Option<Vec<Node<'pr>>> {
    let truthy = unless_node.else_clause().and_then(|e| elide(e.statements()));
    let mut branches = expand_else_position(elide(unless_node.statements()));
    branches.insert(0, truthy);
    branches.into_iter().collect()
}

/// The children of a branch node that is whitequark's `begin` type --
/// unified across Prism's two shapes for it: an implicit multi-statement
/// body (`StatementsNode`) and an explicit parenthesized group
/// (`ParenthesesNode`, itself `None` for `()`, i.e. zero children, or
/// wrapping its own `StatementsNode` otherwise). `None` for anything else
/// (a genuine single expression, upstream's `!begin_type?`).
fn as_begin<'pr>(node: &Node<'pr>) -> Option<(usize, Option<Node<'pr>>, Option<Node<'pr>>)> {
    let body = match node.kind() {
        NodeKind::StatementsNode => Some(node.as_statements_node().expect("kind matched").body()),
        NodeKind::ParenthesesNode => node
            .as_parentheses_node()
            .expect("kind matched")
            .body()
            .and_then(|b| b.as_statements_node())
            .map(|s| s.body()),
        _ => return None,
    };
    Some(match body {
        Some(list) => (list.len(), list.first(), list.last()),
        None => (0, None, None),
    })
}

/// RuboCop's `single_child_branch?`: `!branch.begin_type? || branch.children.size == 1`.
fn single_child_branch(branch: &Node<'_>) -> bool {
    as_begin(branch).is_none_or(|(len, ..)| len == 1)
}

/// RuboCop's `head`: `node.begin_type? ? node.children.first : node`.
fn branch_head<'pr>(branch: &Node<'pr>) -> Option<Node<'pr>> {
    as_begin(branch).map_or_else(|| Some(*branch), |(_, first, _)| first)
}

/// RuboCop's `tail`: `node.begin_type? ? node.children.last : node`.
fn branch_tail<'pr>(branch: &Node<'pr>) -> Option<Node<'pr>> {
    as_begin(branch).map_or_else(|| Some(*branch), |(_, _, last)| last)
}

/// `duplicated_expressions?`: every expression is the same, and -- when
/// that shared expression is an assignment -- the condition does not
/// itself read the assigned value, which would make hoisting change what
/// the condition observes.
fn duplicated_expressions(
    ctx: &Context<'_>,
    condition: Option<&Node<'_>>,
    exprs: &[Option<Node<'_>>],
) -> bool {
    if exprs.is_empty() {
        return false;
    }
    let same = exprs.windows(2).all(|pair| match (&pair[0], &pair[1]) {
        (None, None) => true,
        (Some(a), Some(b)) => expressions_equal(ctx, a, b),
        _ => false,
    });
    if !same {
        return false;
    }
    let Some(unique) = &exprs[0] else { return true };
    if !is_assignment_kind(unique) {
        return true;
    }
    let Some(lhs_span) = dup_guard_lhs(unique, ctx) else { return true };
    let lhs_text = ctx.text(lhs_span);
    let Some(condition) = condition else { return true };
    let mut references_lhs = false;
    for_each_child(condition, |child| {
        if is_variable_read(child.kind()) && ctx.text(child.span()) == lhs_text {
            references_lhs = true;
        }
    });
    !references_lhs
}

/// Node-equality stand-in for RuboCop's structural `Node#==`: same source
/// text, plus -- for a plain (non-interpolated) heredoc anywhere in the
/// subtree -- the same body. A heredoc's own `location` (and that of
/// every ancestor around it, up through the enclosing call/assignment)
/// covers only its opening `<<~TAG` declaration; Prism finalises that
/// span before the lazily-attached, later-positioned body is parsed, so
/// two textually-identical `<<~TAG` openings with different bodies would
/// otherwise compare equal (confirmed false positives against real
/// RuboCop 1.91.0 in the discourse corpus, e.g.
/// `app/helpers/application_helper.rb`'s two `result << <<~HTML` calls).
/// An interpolated heredoc's literal segments are ordinary children with
/// correctly-positioned locations already, so recursing into every child
/// (which also makes this a true deep comparison, not just a heredoc
/// special case) catches them for free.
fn expressions_equal(ctx: &Context<'_>, a: &Node<'_>, b: &Node<'_>) -> bool {
    let mut text_a = Vec::new();
    let mut text_b = Vec::new();
    append_comparable_text(ctx, a, &mut text_a);
    append_comparable_text(ctx, b, &mut text_b);
    text_a == text_b
}

fn append_comparable_text(ctx: &Context<'_>, node: &Node<'_>, out: &mut Vec<u8>) {
    out.extend_from_slice(ctx.text(node.span()));
    if is_heredoc(node) {
        let content_span = match node.kind() {
            NodeKind::StringNode => node.as_string_node().map(|n| n.content_loc().span()),
            NodeKind::XStringNode => node.as_x_string_node().map(|n| n.content_loc().span()),
            _ => None,
        };
        if let Some(span) = content_span {
            out.extend_from_slice(ctx.text(span));
        }
    }
    for_each_child(node, |child| append_comparable_text(ctx, child, out));
}

/// Whitequark's `variable?`: a bare local/instance/class/global variable
/// read (never a constant, which upstream's `VARIABLES` also excludes).
fn is_variable_read(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableReadNode
            | NodeKind::InstanceVariableReadNode
            | NodeKind::ClassVariableReadNode
            | NodeKind::GlobalVariableReadNode
    )
}

/// Whitequark's `EQUALS_ASSIGNMENTS` (`lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`/
/// `casgn`/`masgn`).
fn is_equals_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::MultiWriteNode
    )
}

/// Whitequark's `SHORTHAND_ASSIGNMENTS` (`op_asgn`/`or_asgn`/`and_asgn`),
/// spread across Prism's per-target write kinds.
fn is_shorthand_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::IndexOperatorWriteNode
            | NodeKind::IndexAndWriteNode
            | NodeKind::IndexOrWriteNode
            | NodeKind::CallOperatorWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::CallOrWriteNode
    )
}

/// A `send`/`csend` counted as an assignment by `MethodDispatchNode`'s
/// `alias assignment? setter_method?`: any call carrying an `=` location
/// (`obj.attr = x`, `h[:key] = x`), regardless of receiver.
fn as_setter_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    node.as_call_node().filter(|call| call.equal_loc().is_some())
}

/// `Node#assignment?` as overridden per node shape (see the module doc).
fn is_assignment_kind(node: &Node<'_>) -> bool {
    is_equals_kind(node.kind()) || is_shorthand_kind(node.kind()) || as_setter_call(node).is_some()
}

/// The operator location that starts right after a shorthand assignment's
/// bare target, whichever accessor the specific write kind exposes it
/// under (`binary_operator_loc` for `op_asgn`-shaped writes, `operator_loc`
/// for `or_asgn`/`and_asgn`-shaped ones).
fn shorthand_operator_start(node: &Node<'_>) -> Option<u32> {
    Some(match node.kind() {
        NodeKind::LocalVariableOperatorWriteNode => {
            node.as_local_variable_operator_write_node()?.binary_operator_loc().span().start
        }
        NodeKind::InstanceVariableOperatorWriteNode => {
            node.as_instance_variable_operator_write_node()?.binary_operator_loc().span().start
        }
        NodeKind::ClassVariableOperatorWriteNode => {
            node.as_class_variable_operator_write_node()?.binary_operator_loc().span().start
        }
        NodeKind::GlobalVariableOperatorWriteNode => {
            node.as_global_variable_operator_write_node()?.binary_operator_loc().span().start
        }
        NodeKind::ConstantOperatorWriteNode => {
            node.as_constant_operator_write_node()?.binary_operator_loc().span().start
        }
        NodeKind::ConstantPathOperatorWriteNode => {
            node.as_constant_path_operator_write_node()?.binary_operator_loc().span().start
        }
        NodeKind::IndexOperatorWriteNode => {
            node.as_index_operator_write_node()?.binary_operator_loc().span().start
        }
        NodeKind::CallOperatorWriteNode => {
            node.as_call_operator_write_node()?.binary_operator_loc().span().start
        }
        NodeKind::LocalVariableAndWriteNode => {
            node.as_local_variable_and_write_node()?.operator_loc().span().start
        }
        NodeKind::InstanceVariableAndWriteNode => {
            node.as_instance_variable_and_write_node()?.operator_loc().span().start
        }
        NodeKind::ClassVariableAndWriteNode => {
            node.as_class_variable_and_write_node()?.operator_loc().span().start
        }
        NodeKind::GlobalVariableAndWriteNode => {
            node.as_global_variable_and_write_node()?.operator_loc().span().start
        }
        NodeKind::ConstantAndWriteNode => {
            node.as_constant_and_write_node()?.operator_loc().span().start
        }
        NodeKind::ConstantPathAndWriteNode => {
            node.as_constant_path_and_write_node()?.operator_loc().span().start
        }
        NodeKind::IndexAndWriteNode => node.as_index_and_write_node()?.operator_loc().span().start,
        NodeKind::CallAndWriteNode => node.as_call_and_write_node()?.operator_loc().span().start,
        NodeKind::LocalVariableOrWriteNode => {
            node.as_local_variable_or_write_node()?.operator_loc().span().start
        }
        NodeKind::InstanceVariableOrWriteNode => {
            node.as_instance_variable_or_write_node()?.operator_loc().span().start
        }
        NodeKind::ClassVariableOrWriteNode => {
            node.as_class_variable_or_write_node()?.operator_loc().span().start
        }
        NodeKind::GlobalVariableOrWriteNode => {
            node.as_global_variable_or_write_node()?.operator_loc().span().start
        }
        NodeKind::ConstantOrWriteNode => {
            node.as_constant_or_write_node()?.operator_loc().span().start
        }
        NodeKind::ConstantPathOrWriteNode => {
            node.as_constant_path_or_write_node()?.operator_loc().span().start
        }
        NodeKind::IndexOrWriteNode => node.as_index_or_write_node()?.operator_loc().span().start,
        NodeKind::CallOrWriteNode => node.as_call_or_write_node()?.operator_loc().span().start,
        _ => return None,
    })
}

/// Trims trailing plain spaces/tabs -- the gap between a shorthand
/// assignment's bare target and its operator.
fn trim_trailing_blank(ctx: &Context<'_>, span: Span) -> Span {
    let bytes = ctx.text(span);
    let mut end = span.end;
    for &byte in bytes.iter().rev() {
        if byte == b' ' || byte == b'\t' {
            end -= 1;
        } else {
            break;
        }
    }
    Span::new(span.start, end)
}

/// `duplicated_expressions?`'s `lhs = unique_expression.child_nodes.first`:
/// the RHS value for a plain equals-assignment (its target is a bare
/// symbol/const-name, not a child node), the receiver for a setter call,
/// or the bare target text for a shorthand assignment (`op_asgn` wraps a
/// target-only sub-node as its own first child; a setter's `h[:key]`
/// portion for an indexed shorthand, `self.foo` for an attribute one).
fn dup_guard_lhs(node: &Node<'_>, ctx: &Context<'_>) -> Option<Span> {
    if is_shorthand_kind(node.kind()) {
        let op_start = shorthand_operator_start(node)?;
        return Some(trim_trailing_blank(ctx, Span::new(node.span().start, op_start)));
    }
    if let Some(call) = as_setter_call(node) {
        return Some(call.receiver().map_or(node.span(), |r| r.span()));
    }
    match node.kind() {
        NodeKind::LocalVariableWriteNode => {
            node.as_local_variable_write_node().map(|n| n.value().span())
        }
        NodeKind::InstanceVariableWriteNode => {
            node.as_instance_variable_write_node().map(|n| n.value().span())
        }
        NodeKind::ClassVariableWriteNode => {
            node.as_class_variable_write_node().map(|n| n.value().span())
        }
        NodeKind::GlobalVariableWriteNode => {
            node.as_global_variable_write_node().map(|n| n.value().span())
        }
        NodeKind::ConstantWriteNode => node.as_constant_write_node().map(|n| n.value().span()),
        NodeKind::ConstantPathWriteNode => {
            node.as_constant_path_write_node().map(|n| n.value().span())
        }
        NodeKind::MultiWriteNode => node.as_multi_write_node().map(|n| n.value().span()),
        _ => None,
    }
}

/// `check_branches`' own `assigned_value`, restricted to the two shapes
/// where upstream's `node_parts[0].to_s`/`head.receiver.source` yields
/// plain, potentially-matching text (see the module doc for why every
/// other assignment shape is permanently inert upstream and simply
/// produces `None` here instead).
fn assigned_value_span(node: &Node<'_>) -> Option<Span> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => {
            node.as_local_variable_write_node().map(|n| n.name_loc().span())
        }
        NodeKind::InstanceVariableWriteNode => {
            node.as_instance_variable_write_node().map(|n| n.name_loc().span())
        }
        NodeKind::ClassVariableWriteNode => {
            node.as_class_variable_write_node().map(|n| n.name_loc().span())
        }
        NodeKind::GlobalVariableWriteNode => {
            node.as_global_variable_write_node().map(|n| n.name_loc().span())
        }
        NodeKind::CallNode => node
            .as_call_node()
            .filter(|call| !call.is_safe_navigation() && call.equal_loc().is_some())
            .map(|call| call.receiver().map_or(node.span(), |r| r.span())),
        _ => None,
    }
}

/// `assignable_condition_value`: the condition's receiver (or its whole
/// source, for a receiver-less call) for a call-type condition, or the
/// condition's own source for a bare variable read.
fn assignable_condition_value(condition: &Node<'_>) -> Option<Span> {
    if let Some(call) = condition.as_call_node() {
        return Some(call.receiver().map_or(condition.span(), |r| r.span()));
    }
    if is_variable_read(condition.kind()) {
        return Some(condition.span());
    }
    None
}

/// The whitespace-only prefix of `offset`'s own line, as a byte count --
/// RuboCop's `indentation_of`, which reuses the conditional's (or its
/// assignment's) own column rather than inspecting actual leading bytes.
fn indent_spaces(ctx: &Context<'_>, offset: u32) -> Vec<u8> {
    vec![b' '; ctx.line_col(offset).column as usize]
}

/// `correct_assignment`/`correct_no_assignment`: the extra edit(s), beyond
/// every occurrence's own line deletion, that actually place the hoisted
/// expression -- emitted once, for the first occurrence only.
fn insertion_edits(
    ctx: &Context<'_>,
    node_span: Span,
    parent: Option<NodeInfo>,
    expr: &Node<'_>,
    position: InsertPosition,
) -> Vec<Edit> {
    let expr_text = ctx.text(expr.span());
    let assignment_parent = parent.filter(|p| is_equals_kind(p.kind) || is_shorthand_kind(p.kind));
    match (assignment_parent, position) {
        (Some(parent), InsertPosition::AfterCondition) => {
            let indent = indent_spaces(ctx, parent.span.start);
            let assignment_span = Span::new(parent.span.start, node_span.start);
            let mut replacement = Vec::with_capacity(1 + indent.len() + expr_text.len() * 2);
            replacement.push(b'\n');
            replacement.extend_from_slice(&indent);
            replacement.extend_from_slice(ctx.text(assignment_span));
            replacement.extend_from_slice(expr_text);
            vec![Edit::delete(assignment_span), Edit::insert(node_span.end, replacement)]
        }
        (Some(parent), InsertPosition::BeforeCondition) => {
            let indent = indent_spaces(ctx, parent.span.start);
            let mut replacement = Vec::with_capacity(expr_text.len() + 1 + indent.len());
            replacement.extend_from_slice(expr_text);
            replacement.push(b'\n');
            replacement.extend_from_slice(&indent);
            vec![Edit::insert(parent.span.start, replacement)]
        }
        (None, InsertPosition::AfterCondition) => {
            let indent = indent_spaces(ctx, node_span.start);
            let mut replacement = Vec::with_capacity(1 + indent.len() + expr_text.len());
            replacement.push(b'\n');
            replacement.extend_from_slice(&indent);
            replacement.extend_from_slice(expr_text);
            vec![Edit::insert(node_span.end, replacement)]
        }
        (None, InsertPosition::BeforeCondition) => {
            let indent = indent_spaces(ctx, node_span.start);
            let mut replacement = Vec::with_capacity(expr_text.len() + 1 + indent.len());
            replacement.extend_from_slice(expr_text);
            replacement.push(b'\n');
            replacement.extend_from_slice(&indent);
            vec![Edit::insert(node_span.start, replacement)]
        }
    }
}
