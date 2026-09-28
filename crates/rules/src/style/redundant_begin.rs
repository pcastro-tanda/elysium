//! `Style/RedundantBegin`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_begin.rb`.
//!
//! Prism always wraps a body that has at least one statement in a
//! `StatementsNode`, even when it holds exactly one statement, unlike
//! whitequark's AST, which elides the wrapper for a single-statement body
//! and exposes the lone statement's real syntactic parent directly (the
//! `def`/`if`/`while`/... node, or nothing at all for the file's own sole
//! top-level statement). This port re-derives that "real parent" by
//! skip-through: whenever a `StatementsNode` holds exactly one statement,
//! that statement's logical parent is whatever directly contains the
//! `StatementsNode`; whenever it holds zero or several, the logical parent
//! is opaque (matching whitequark's own anonymous multi-statement sequence
//! node, which never satisfies any of `assignment?`/`post_condition_loop?`/
//! `send_type?`/`operator_keyword?` either) -- see `Parent`.
//!
//! Upstream's generic `on_kwbegin` handler additionally searches the *whole
//! subtree* of every `kwbegin` for a non-allowable nested `kwbegin` and, if
//! one exists, reports that one instead of the outer node (`.last` of
//! `offensive_kwbegins`) -- so a `kwbegin` whose only child is itself an
//! offending `kwbegin` (chained `||=` assignments, say) is silently skipped
//! in favour of the inner one, which gets reported through its own
//! (separate) `on_kwbegin` call. [`contains_non_allowable_descendant`]
//! reproduces that suppression; already-registered offenses are
//! deduplicated by keyword span since a node reached through both a
//! specific handler (`on_def`/`on_if`/...) and the generic fallback would
//! otherwise be registered twice, exactly as it would be by two separate
//! `on_*` callbacks upstream.

use std::collections::HashSet;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::is_lambda_or_proc;
use ruby_ast::node::{BeginNode, StatementsNode};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// RuboCop's `MSG`.
const MSG: &str = "Redundant `begin` block detected.";

/// Checks for redundant `begin` blocks.
#[derive(Debug, Clone)]
pub struct RedundantBegin {
    target_ruby_version: f32,
    /// Keyword-span starts already reported, so a `kwbegin` reached through
    /// both a specific container handler and the generic fallback (exactly
    /// as upstream's `on_def`/`on_kwbegin` both fire on the same node) is
    /// only reported once.
    reported: HashSet<u32>,
}

impl Rule for RedundantBegin {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantBegin",
        department: Department::Style,
        summary: "Don't use begin blocks when they are not needed.",
        explanation: "\
Checks for redundant `begin` blocks.

Currently it checks for code like this:

```ruby
# bad
def redundant
  begin
    ala
    bala
  rescue StandardError => e
    something
  end
end

# good
def preferred
  ala
  bala
rescue StandardError => e
  something
end

# bad
begin
  do_something
end

# good
do_something

# bad
# When using Ruby 2.5 or later.
do_something do
  begin
    something
  rescue => ex
    anything
  end
end

# good
# In Ruby 2.5 or later, you can omit `begin` in `do-end` block.
do_something do
  something
rescue => ex
  anything
end

# good
# Stabby lambdas don't support implicit `begin` in `do-end` blocks.
-> do
  begin
    foo
  rescue Bar
    baz
  end
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version(), reported: HashSet::new() })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let Some(program) = root.as_program_node() else { return };
        let statements = program.statements();
        self.walk_statements(&statements, None, ctx);
    }
}

/// A node's logical (whitequark-shaped) parent while walking the tree.
#[derive(Clone, Copy)]
enum Parent<'pr> {
    /// whitequark's `node.parent == nil`: this is the file's own sole
    /// top-level statement, nothing wraps it at all.
    None,
    /// whitequark's anonymous multi-statement sequence node: present, but
    /// never an assignment/loop/call/operator, so every `Parent` predicate
    /// below is false for it.
    Opaque,
    /// A genuine enclosing construct.
    Real(Node<'pr>),
}

impl Parent<'_> {
    fn is_none(&self) -> bool {
        matches!(self, Parent::None)
    }

    /// RuboCop's `Node#assignment?`.
    fn is_assignment(&self) -> bool {
        let Parent::Real(n) = self else { return false };
        is_assignment_kind(n.kind()) || n.as_call_node().is_some_and(|c| c.equal_loc().is_some())
    }

    /// RuboCop's `Node#post_condition_loop?`: a `begin...end while`/`until`.
    fn is_post_condition_loop(&self) -> bool {
        match self {
            Parent::Real(n) => match n.kind() {
                NodeKind::WhileNode => n.as_while_node().is_some_and(|w| w.is_begin_modifier()),
                NodeKind::UntilNode => n.as_until_node().is_some_and(|w| w.is_begin_modifier()),
                _ => false,
            },
            _ => false,
        }
    }

    /// RuboCop's `Node#send_type?`, used as `begin...end` in argument or
    /// receiver position.
    fn is_send(&self) -> bool {
        matches!(self, Parent::Real(n) if n.kind() == NodeKind::CallNode)
    }

    /// RuboCop's `Node#operator_keyword?` (`and`/`or`).
    fn is_operator_keyword(&self) -> bool {
        matches!(self, Parent::Real(n) if matches!(n.kind(), NodeKind::AndNode | NodeKind::OrNode))
    }
}

/// RuboCop-AST's `ASGN_NODES`-based `Node#assignment?`, translated to the
/// Prism node kinds it covers: every local/instance/class/global/constant/
/// constant-path write (plain, `&&=`, `||=`, `op=`), attribute/index
/// compound writes, and multiple assignment. Plain attribute/index
/// assignment (`obj.attr = x`, `a[i] = x`) is a `CallNode` with `equal_loc`
/// set instead, handled separately in `Parent::is_assignment`.
fn is_assignment_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::CallOperatorWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::CallOrWriteNode
            | NodeKind::IndexOperatorWriteNode
            | NodeKind::IndexAndWriteNode
            | NodeKind::IndexOrWriteNode
            | NodeKind::MultiWriteNode
    )
}

/// The number of children `b` would have in whitequark: `1` whenever a
/// `rescue`/`ensure` clause is present (the whole body nests under a single
/// `rescue`/`ensure` node there), else the number of statements.
fn child_count(b: &BeginNode<'_>) -> usize {
    if b.rescue_clause().is_some() || b.ensure_clause().is_some() {
        1
    } else {
        b.statements().map_or(0, |s| s.body().len())
    }
}

/// RuboCop's `RedundantBegin#allowable_kwbegin?`.
fn is_allowable(b: &BeginNode<'_>, parent: &Parent<'_>) -> bool {
    let has_rescue_or_ensure = b.rescue_clause().is_some() || b.ensure_clause().is_some();
    let count = child_count(b);
    let empty = count == 0 && !has_rescue_or_ensure;
    let multiline_statements = !parent.is_none() && count >= 2;
    let valid_begin_assignment = parent.is_assignment() && count != 1;
    empty
        || multiline_statements
        || has_rescue_or_ensure
        || valid_begin_assignment
        || parent.is_post_condition_loop()
        || parent.is_send()
        || parent.is_operator_keyword()
}

/// `IfNode`/`UnlessNode` written in modifier form (`expr if cond`, `expr
/// unless cond`): no `end` keyword. Ternaries also lack `end`, so `IfNode`
/// additionally requires an actual `if` keyword to exclude them, matching
/// `IfNode#modifier_form?`'s `if? || unless?` guard.
fn is_modifier_conditional(n: &Node<'_>) -> bool {
    match n.kind() {
        NodeKind::IfNode => n
            .as_if_node()
            .is_some_and(|i| i.end_keyword_loc().is_none() && i.if_keyword_loc().is_some()),
        NodeKind::UnlessNode => n.as_unless_node().is_some_and(|u| u.end_keyword_loc().is_none()),
        _ => false,
    }
}

/// RuboCop's `condition_range`: the `if`/`unless` keyword through the end of
/// its condition.
fn modifier_condition_span(n: &Node<'_>) -> Option<Span> {
    match n.kind() {
        NodeKind::IfNode => {
            let i = n.as_if_node()?;
            Some(Span::new(i.if_keyword_loc()?.span().start, i.predicate().span().end))
        }
        NodeKind::UnlessNode => {
            let u = n.as_unless_node()?;
            Some(Span::new(u.keyword_loc().span().start, u.predicate().span().end))
        }
        _ => None,
    }
}

/// The lone statement of a `begin` node's own body (never a `rescue`/
/// `ensure` clause: those are exposed separately).
fn first_stmt<'pr>(b: &BeginNode<'pr>) -> Option<Node<'pr>> {
    b.statements().and_then(|s| s.body().first())
}

/// Whether `stmts` (a construct's body/branch) is exactly one explicit
/// `begin...end`, unwrapping Prism's always-present `StatementsNode`.
fn single_stmt_begin(stmts: Option<StatementsNode<'_>>) -> Option<(BeginNode<'_>, Node<'_>)> {
    let stmts = stmts?;
    if stmts.body().len() != 1 {
        return None;
    }
    let only = stmts.body().first()?;
    let b = only.as_begin_node()?;
    if b.begin_keyword_loc().is_some() {
        Some((b, only))
    } else {
        None
    }
}

/// RuboCop's `IfNode#branches`, flattening the `elsif` chain.
fn if_branches<'pr>(node: &Node<'pr>) -> Vec<Option<StatementsNode<'pr>>> {
    let mut result = Vec::new();
    let mut current = *node;
    while let Some(i) = current.as_if_node() {
        result.push(i.statements());
        match i.subsequent() {
            None => break,
            Some(sub) if sub.kind() == NodeKind::IfNode => {
                current = sub;
            }
            Some(sub) => {
                if let Some(e) = sub.as_else_node() {
                    result.push(e.statements());
                }
                break;
            }
        }
    }
    result
}

/// RuboCop's `IfNode#branches` for `unless` (no `elsif` chaining possible).
fn unless_branches<'pr>(node: &Node<'pr>) -> Vec<Option<StatementsNode<'pr>>> {
    let u = node.as_unless_node().expect("kind matched");
    let mut result = vec![u.statements()];
    if let Some(e) = u.else_clause() {
        result.push(e.statements());
    }
    result
}

/// Every direct child of `node`, walking through Prism's `StatementsNode`
/// transparently so a candidate `kwbegin` reachable *anywhere* below (any
/// depth, any node kind) is still found -- upstream's `offensive_kwbegins`
/// searches the whole subtree, not just direct bodies.
fn contains_non_allowable_descendant(node: &Node<'_>) -> bool {
    let mut kids = Vec::new();
    for_each_child(node, |c| kids.push(*c));
    for c in kids {
        if let Some(stmts) = c.as_statements_node() {
            let body = stmts.body();
            let len = body.len();
            for grand in &body {
                let p = if len == 1 { Parent::Real(*node) } else { Parent::Opaque };
                if check_descendant(&grand, &p) {
                    return true;
                }
            }
        } else if check_descendant(&c, &Parent::Real(*node)) {
            return true;
        }
    }
    false
}

fn check_descendant(n: &Node<'_>, p: &Parent<'_>) -> bool {
    if let Some(b) = n.as_begin_node() {
        if b.begin_keyword_loc().is_some() && !is_allowable(&b, p) {
            return true;
        }
    }
    contains_non_allowable_descendant(n)
}

impl RedundantBegin {
    fn walk_statements(
        &mut self,
        stmts: &StatementsNode<'_>,
        enclosing: Option<Node<'_>>,
        ctx: &mut Context<'_>,
    ) {
        let body = stmts.body();
        let len = body.len();
        for child in &body {
            let parent = if len == 1 {
                match &enclosing {
                    Some(n) => Parent::Real(*n),
                    None => Parent::None,
                }
            } else {
                Parent::Opaque
            };
            self.walk(&child, parent, ctx);
        }
    }

    fn walk(&mut self, node: &Node<'_>, parent: Parent<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode => self.visit_def(node, ctx),
            NodeKind::IfNode => self.visit_if(node, ctx),
            NodeKind::UnlessNode => self.visit_unless(node, ctx),
            NodeKind::CaseNode => self.visit_case(node, ctx),
            NodeKind::CaseMatchNode => self.visit_case_match(node, ctx),
            NodeKind::WhileNode => self.visit_while(node, ctx),
            NodeKind::UntilNode => self.visit_until(node, ctx),
            NodeKind::BlockNode => self.visit_block(node, &parent, ctx),
            NodeKind::BeginNode => self.visit_generic_begin(node, &parent, ctx),
            _ => {}
        }
        self.recurse_children(node, ctx);
    }

    fn recurse_children(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let mut kids = Vec::new();
        for_each_child(node, |c| kids.push(*c));
        for c in kids {
            if let Some(stmts) = c.as_statements_node() {
                self.walk_statements(&stmts, Some(*node), ctx);
            } else {
                self.walk(&c, Parent::Real(*node), ctx);
            }
        }
    }

    /// RuboCop's `on_kwbegin`, narrowed to the single candidate `node`
    /// (already known to be a `kwbegin`): register it unless it is
    /// allowable, or a non-allowable `kwbegin` exists deeper in its subtree
    /// and will be reported through its own visit instead.
    fn visit_generic_begin(&mut self, node: &Node<'_>, parent: &Parent<'_>, ctx: &mut Context<'_>) {
        let b = node.as_begin_node().expect("kind matched");
        if b.begin_keyword_loc().is_none() {
            return;
        }
        if is_allowable(&b, parent) {
            return;
        }
        if contains_non_allowable_descendant(node) {
            return;
        }
        self.register_offense(&b, node, parent, ctx);
    }

    /// RuboCop's `on_def`/`on_defs`.
    fn visit_def(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let d = node.as_def_node().expect("kind matched");
        if d.equal_loc().is_some() {
            return; // endless method: handled by the generic kwbegin route
        }
        let stmts = d.body().and_then(|n| n.as_statements_node());
        self.register_branch(stmts, Parent::Real(*node), ctx, false);
    }

    /// RuboCop's `on_if`.
    fn visit_if(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if is_modifier_conditional(node) {
            return;
        }
        for stmts in if_branches(node) {
            self.register_branch(stmts, Parent::Real(*node), ctx, true);
        }
    }

    /// RuboCop's `on_if` applied to `unless` (whitequark aliases the same
    /// callback; Prism gives `unless` its own node kind).
    fn visit_unless(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if is_modifier_conditional(node) {
            return;
        }
        for stmts in unless_branches(node) {
            self.register_branch(stmts, Parent::Real(*node), ctx, true);
        }
    }

    /// RuboCop's `on_case`/`on_case_match`.
    fn visit_case(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let c = node.as_case_node().expect("kind matched");
        for w in &c.conditions() {
            if let Some(when) = w.as_when_node() {
                self.register_branch(when.statements(), Parent::Real(w), ctx, true);
            }
        }
        if let Some(e) = c.else_clause() {
            self.register_branch(e.statements(), Parent::Real(*node), ctx, true);
        }
    }

    /// RuboCop's `on_case_match`.
    fn visit_case_match(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let c = node.as_case_match_node().expect("kind matched");
        for w in &c.conditions() {
            if let Some(in_node) = w.as_in_node() {
                self.register_branch(in_node.statements(), Parent::Real(w), ctx, true);
            }
        }
        if let Some(e) = c.else_clause() {
            self.register_branch(e.statements(), Parent::Real(*node), ctx, true);
        }
    }

    /// RuboCop's `on_while`/`on_until`.
    fn visit_while(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let w = node.as_while_node().expect("kind matched");
        if w.closing_loc().is_none() {
            return; // modifier form (`x while y`), including the `begin...end while` shape
        }
        self.register_branch(w.statements(), Parent::Real(*node), ctx, true);
    }

    fn visit_until(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let u = node.as_until_node().expect("kind matched");
        if u.closing_loc().is_none() {
            return;
        }
        self.register_branch(u.statements(), Parent::Real(*node), ctx, true);
    }

    /// RuboCop's `on_block`/`on_numblock`/`on_itblock`.
    fn visit_block(&mut self, node: &Node<'_>, parent: &Parent<'_>, ctx: &mut Context<'_>) {
        if self.target_ruby_version < 2.5 {
            return;
        }
        if let Parent::Real(p) = parent {
            if let Some(call) = p.as_call_node() {
                if is_lambda_or_proc(&call) {
                    return;
                }
            }
        }
        let b = node.as_block_node().expect("kind matched");
        if ctx.text(b.opening_loc().span()).first() == Some(&b'{') {
            return; // braces? -- `do`/`end` blocks only
        }
        let stmts = b.body().and_then(|n| n.as_statements_node());
        self.register_branch(stmts, Parent::Real(*node), ctx, false);
    }

    /// Looks for exactly one explicit `begin...end` in `stmts` and registers
    /// it (optionally requiring it have no `rescue`/`ensure`, matching
    /// `on_if`/`on_case`/`on_while`'s stricter filter versus `on_def`/
    /// `on_block`'s unconditional one).
    fn register_branch(
        &mut self,
        stmts: Option<StatementsNode<'_>>,
        parent: Parent<'_>,
        ctx: &mut Context<'_>,
        require_no_rescue_or_ensure: bool,
    ) {
        let Some((b, n)) = single_stmt_begin(stmts) else { return };
        if require_no_rescue_or_ensure
            && (b.rescue_clause().is_some() || b.ensure_clause().is_some())
        {
            return;
        }
        self.register_offense(&b, &n, &parent, ctx);
    }

    /// RuboCop's `register_offense` plus its corrector: replaces the
    /// `begin` keyword with the lone statement's own source when the
    /// `begin...end` is itself an assignment's value (preserving any
    /// comments that sat between `begin` and the statement, moved above the
    /// assignment), otherwise just deletes `begin`/`end`; additionally
    /// hoists a multiline block's trailing modifier `if`/`unless` onto its
    /// last statement once the wrapping `begin...end` disappears.
    fn register_offense(
        &mut self,
        b: &BeginNode<'_>,
        node: &Node<'_>,
        parent: &Parent<'_>,
        ctx: &mut Context<'_>,
    ) {
        let Some(begin_kw) = b.begin_keyword_loc() else { return };
        let offense_span = begin_kw.span();
        if !self.reported.insert(offense_span.start) {
            return;
        }

        let mut edits: Vec<Edit> = Vec::new();

        if parent.is_assignment() {
            if let Some(first) = first_stmt(b) {
                let mut source = ctx.text(first.span()).to_vec();
                if is_modifier_conditional(&first) {
                    let mut wrapped = vec![b'('];
                    wrapped.extend_from_slice(&source);
                    wrapped.push(b')');
                    source = wrapped;
                }
                edits.push(Edit::replace(offense_span, source));
                edits.push(Edit::delete(Span::new(offense_span.end, first.span().end)));

                let comments_span = Span::new(offense_span.end, first.span().start);
                let comments_text = ctx.text(comments_span);
                if !comments_text.iter().all(u8::is_ascii_whitespace) {
                    if let Parent::Real(p) = parent {
                        edits.push(Edit::insert(p.span().start, comments_text.to_vec()));
                    }
                }
            }
        } else {
            let mut off = offense_span;
            if let Parent::Real(p) = parent {
                if p.kind() == NodeKind::DefNode
                    && p.as_def_node().is_some_and(|d| d.equal_loc().is_some())
                {
                    off = ctx.with_surrounding_space(off, Side::Both, true, false);
                }
            }
            edits.push(Edit::delete(off));
        }

        let mut whole_line_covers_end = false;
        if let Parent::Real(p) = parent {
            if is_modifier_conditional(p) && !ctx.is_single_line(node.span()) {
                if let (Some(cond_span), Some(first)) = (modifier_condition_span(p), first_stmt(b))
                {
                    let cond_text = ctx.text(cond_span).to_vec();
                    let mut insertion = vec![b' '];
                    insertion.extend_from_slice(&cond_text);
                    edits.push(Edit::insert(first.span().end, insertion));
                    edits.push(Edit::delete(ctx.whole_lines(cond_span)));
                    whole_line_covers_end = true;
                }
            }
        }

        if !whole_line_covers_end {
            if let Some(end_kw) = b.end_keyword_loc() {
                edits.push(Edit::delete(end_kw.span()));
            }
        }

        ctx.report_with_fix(
            &Self::META,
            offense_span,
            MSG,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}
