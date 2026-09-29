//! `Style/RedundantAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_assignment.rb`.
//!
//! # Branch shapes
//!
//! Upstream walks a method body recursively, dispatching on whitequark node
//! type (`:case`, `:case_match`, `:if`, `:rescue`/`:resbody`, `:ensure`,
//! `:begin`/`:kwbegin`) down to the final statement of each leaf branch,
//! looking for a `(lvasgn name expr)` immediately followed by a matching
//! `(lvar name)` as the last two statements. Prism always wraps a branch
//! body in a [`StatementsNode`] (never eliding it for a single statement, or
//! omitting it for zero), so [`check_statements`] is the single leaf case
//! every branch dispatch bottoms out at, and [`check_branch`] is the
//! recursive dispatcher matching upstream's `check_branch`.
//!
//! `IfNode::end_keyword_loc` is `None` for both a ternary (whitequark's
//! `ternary?`) and a modifier `if`/`unless` (whitequark's `modifier_form?`)
//! -- neither has a trailing `end` -- while a real `elsif` link reached
//! through `subsequent()` shares its head's `end_keyword_loc`, so this one
//! check replaces both upstream guards.
//!
//! A `def`/`defs` body with a `rescue` and/or `ensure` clause is an implicit
//! [`BeginNode`] in Prism (never a bare `StatementsNode`). Upstream's
//! `:ensure` dispatch only ever calls `check_branch` on the ensure clause's
//! *own* body (`EnsureNode#branch`), never on the protected body it
//! guards -- so when a `BeginNode` has an `ensure_clause`, this port checks
//! only that clause and stops, matching
//! `does_not_register_an_offense_when_ensure_block_present`. Absent an
//! `ensure_clause`, a `rescue_clause` present means the top `statements()` is
//! the protected body (upstream's `:rescue` node's first child), walked
//! alongside the `rescue_clause`/`subsequent()` chain (each link is one
//! `resbody`) and the rescue's own `else_clause`. With neither clause, the
//! `BeginNode` is a plain `begin...end` (whitequark's bare `:kwbegin`) and
//! only its `statements()` are checked.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BeginNode, RescueNode, StatementsNode};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Redundant assignment before returning detected.";

/// Checks for redundant assignment before returning.
#[derive(Debug, Clone)]
pub struct RedundantAssignment;

impl Rule for RedundantAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantAssignment",
        department: Department::Style,
        summary: "Checks for redundant assignment before returning.",
        explanation: "\
Checks for redundant assignment before returning.

```ruby
# bad
def test
  x = foo
  x
end

# bad
def test
  if x
    z = foo
    z
  elsif y
    z = bar
    z
  end
end

# good
def test
  foo
end

# good
def test
  if x
    foo
  elsif y
    bar
  end
end
```

When there are comments between the assignment and reference, the cop will
report an offense but it will not autocorrect.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let def = node.as_def_node().expect("kind matched");
        check_branch(def.body(), ctx);
    }
}

/// RuboCop's `check_branch`: dispatches on a branch node's own kind, walking
/// down to every leaf [`StatementsNode`] the branch can end with.
fn check_branch(node: Option<Node<'_>>, ctx: &mut Context<'_>) {
    let Some(node) = node else { return };
    match node.kind() {
        NodeKind::StatementsNode => {
            check_statements(node.as_statements_node().expect("kind matched"), ctx);
        }
        NodeKind::CaseNode => {
            let case = node.as_case_node().expect("kind matched");
            for condition in &case.conditions() {
                let when = condition.as_when_node().expect("case condition is a WhenNode");
                if let Some(statements) = when.statements() {
                    check_statements(statements, ctx);
                }
            }
            if let Some(else_clause) = case.else_clause() {
                if let Some(statements) = else_clause.statements() {
                    check_statements(statements, ctx);
                }
            }
        }
        NodeKind::CaseMatchNode => {
            let case = node.as_case_match_node().expect("kind matched");
            for condition in &case.conditions() {
                let in_node = condition.as_in_node().expect("case/in condition is an InNode");
                if let Some(statements) = in_node.statements() {
                    check_statements(statements, ctx);
                }
            }
            if let Some(else_clause) = case.else_clause() {
                if let Some(statements) = else_clause.statements() {
                    check_statements(statements, ctx);
                }
            }
        }
        NodeKind::IfNode => {
            let if_node = node.as_if_node().expect("kind matched");
            // A ternary has no `if`/`elsif` keyword; a modifier `if`/
            // `unless` (whitequark's `modifier_form?`) has no trailing
            // `end`. Both leave `end_keyword_loc` unset; a real `elsif`
            // link (reached below through `subsequent()`) shares its
            // head's `end_keyword_loc`, so it is never excluded here.
            if if_node.end_keyword_loc().is_none() {
                return;
            }
            if let Some(statements) = if_node.statements() {
                check_statements(statements, ctx);
            }
            check_branch(if_node.subsequent(), ctx);
        }
        NodeKind::ElseNode => {
            let else_node = node.as_else_node().expect("kind matched");
            if let Some(statements) = else_node.statements() {
                check_statements(statements, ctx);
            }
        }
        NodeKind::BeginNode => check_begin_node(node.as_begin_node().expect("kind matched"), ctx),
        _ => {}
    }
}

/// RuboCop's `check_begin_node`, generalised to Prism's implicit
/// [`BeginNode`] for a `rescue`/`ensure`-bearing body (see the module doc).
fn check_begin_node(begin: BeginNode<'_>, ctx: &mut Context<'_>) {
    if let Some(ensure_clause) = begin.ensure_clause() {
        // Upstream's `:ensure` dispatch never looks at the protected body,
        // only the ensure clause's own (`EnsureNode#branch`).
        if let Some(statements) = ensure_clause.statements() {
            check_statements(statements, ctx);
        }
        return;
    }
    if let Some(statements) = begin.statements() {
        check_statements(statements, ctx);
    }
    if let Some(rescue_clause) = begin.rescue_clause() {
        check_rescue_chain(rescue_clause, ctx);
        if let Some(else_clause) = begin.else_clause() {
            if let Some(statements) = else_clause.statements() {
                check_statements(statements, ctx);
            }
        }
    }
}

/// Walks one `rescue`/`resbody` chain, checking each link's own body.
fn check_rescue_chain(rescue: RescueNode<'_>, ctx: &mut Context<'_>) {
    if let Some(statements) = rescue.statements() {
        check_statements(statements, ctx);
    }
    if let Some(next) = rescue.subsequent() {
        check_rescue_chain(next, ctx);
    }
}

/// The leaf case: a branch body's own statement list. RuboCop's
/// `redundant_assignment?` node pattern (matching when the *last two*
/// statements are a local-variable assignment immediately followed by a
/// read of that same variable) plus, when it does not match, the
/// `last_expr = node.children.last; check_branch(last_expr)` fallback --
/// recursing into the final statement in case it is itself a nested
/// control-flow branch (e.g. an `if` as the last statement of a `begin`).
fn check_statements(statements: StatementsNode<'_>, ctx: &mut Context<'_>) {
    let body = statements.body();
    let len = body.len();
    if len == 0 {
        return;
    }
    if len >= 2 {
        let mut it = body.iter();
        let assignment = it.by_ref().nth(len - 2).expect("len checked above");
        let reference = it.next().expect("len checked above");
        if let (Some(write), Some(read)) =
            (assignment.as_local_variable_write_node(), reference.as_local_variable_read_node())
        {
            if write.name().as_slice() == read.name().as_slice() {
                report(&write, &reference, ctx);
                return;
            }
        }
    }
    check_branch(body.last(), ctx);
}

/// Reports the offense and, unless a comment sits between the assignment
/// and its reference, attaches the autocorrection: replace the assignment
/// with its own right-hand side, and delete the now-redundant reference.
fn report(
    write: &ruby_ast::node::LocalVariableWriteNode<'_>,
    reference: &Node<'_>,
    ctx: &mut Context<'_>,
) {
    let assignment_span = write.as_node().span();
    if comments_between(assignment_span, reference.span(), ctx) {
        ctx.report(&RedundantAssignment::META, assignment_span, MSG);
        return;
    }
    let value_span = write.value().span();
    let value_text = ctx.text(value_span).to_vec();
    ctx.report_with_fix(
        &RedundantAssignment::META,
        assignment_span,
        MSG,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(assignment_span, value_text), Edit::delete(reference.span())],
        },
    );
}

/// RuboCop's `comments_between_assignment_and_reference?`: line-based (any
/// comment on a line from the assignment's own line through the
/// reference's own line, inclusive) -- see [`Context::in_opaque_span`]'s
/// sibling note on `contains_comment?` in the crate-level trap list.
fn comments_between(assignment_span: Span, reference_span: Span, ctx: &Context<'_>) -> bool {
    let start_line = ctx.line_col(assignment_span.start).line;
    let end_line = ctx.line_col(reference_span.start).line;
    ctx.comments().iter().any(|comment| comment.line >= start_line && comment.line <= end_line)
}
