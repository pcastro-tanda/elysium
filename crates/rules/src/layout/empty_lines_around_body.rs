//! Shared plumbing for RuboCop's `EmptyLinesAroundBody` mixin
//! (`lib/rubocop/cop/mixin/empty_lines_around_body.rb`), reused by
//! `Layout/EmptyLinesAroundBeginBody`, `Layout/EmptyLinesAroundBlockBody`,
//! `Layout/EmptyLinesAroundMethodBody`, and `Layout/EmptyLinesAroundModuleBody`.
//!
//! `Layout/EmptyLinesAroundClassBody` predates this file and keeps its own
//! self-contained copy of the same logic (it alone needs the
//! `beginning_only`/`ending_only` styles that the other four cops' configs
//! never expose); `Layout/EmptyLinesAroundExceptionHandlingKeywords` doesn't
//! use this mixin at all upstream, and is ported independently.
//!
//! Prism always wraps a multi-statement body in a
//! [`NodeKind::StatementsNode`], even when it holds exactly one statement --
//! unlike the whitequark AST RuboCop's source is written against, where a
//! single-statement body is that statement node directly (no wrapping
//! `begin` node). [`BodyShape`] renormalizes this: a one-child
//! `StatementsNode` unwraps to that child (whitequark's non-`begin_type?`
//! body), anything else keeps every child (whitequark's `begin_type?` body).

use linter::{Applicability, Context, Edit, Fix, RuleMeta};
use ruby_ast::{LocationExt as _, Node};
use ruby_source::Span;

/// Whether a boundary (beginning or end) wants a blank line present
/// (`:empty_lines`) or forbidden (`:no_empty_lines`) -- RuboCop's plain
/// `:empty_lines`/`:no_empty_lines` style values, as passed directly to
/// `check_both`/`check_beginning`/`check_ending`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Want {
    Empty,
    NoEmpty,
}

/// A normalized view of a body, mirroring whitequark's `begin_type?` split
/// (see the module doc comment).
pub(crate) enum BodyShape<'pr> {
    Single(Node<'pr>),
    Multi(Vec<Node<'pr>>),
}

/// Renormalizes a Prism body (always a [`ruby_ast::NodeKind::StatementsNode`]
/// when present, even for one statement) into whitequark's `begin_type?`
/// split; see the module doc comment.
pub(crate) fn shape_of(body: Node<'_>) -> BodyShape<'_> {
    if let Some(stmts) = body.as_statements_node() {
        let mut items: Vec<Node<'_>> = stmts.body().iter().collect();
        if items.len() == 1 {
            BodyShape::Single(items.pop().expect("len == 1"))
        } else {
            BodyShape::Multi(items)
        }
    } else {
        BodyShape::Single(body)
    }
}

/// RuboCop's `namespace?`.
pub(crate) fn is_namespace(shape: &BodyShape<'_>, with_one_child: bool) -> bool {
    match shape {
        BodyShape::Multi(children) => {
            if with_one_child {
                false
            } else {
                children.iter().all(is_constant_definition)
            }
        }
        BodyShape::Single(node) => is_constant_definition(node),
    }
}

/// RuboCop's `first_child_requires_empty_line?`.
pub(crate) fn first_child_requires_empty_line(shape: &BodyShape<'_>) -> bool {
    match shape {
        BodyShape::Multi(children) => children.first().is_some_and(is_empty_line_required),
        BodyShape::Single(node) => is_empty_line_required(node),
    }
}

/// RuboCop's `first_empty_line_required_child`.
pub(crate) fn first_empty_line_required_child<'a, 'pr>(
    shape: &'a BodyShape<'pr>,
) -> Option<&'a Node<'pr>> {
    match shape {
        BodyShape::Multi(children) => children.iter().find(|c| is_empty_line_required(c)),
        BodyShape::Single(node) => is_empty_line_required(node).then_some(node),
    }
}

/// RuboCop's `constant_definition?`: `{class module}`.
fn is_constant_definition(node: &Node<'_>) -> bool {
    matches!(node, Node::ClassNode { .. } | Node::ModuleNode { .. })
}

/// RuboCop's `empty_line_required?`:
/// `{any_def class module (send nil? {:private :protected :public})}`.
fn is_empty_line_required(node: &Node<'_>) -> bool {
    match node {
        Node::DefNode { .. } | Node::ClassNode { .. } | Node::ModuleNode { .. } => true,
        Node::CallNode { .. } => is_bare_access_modifier(node),
        _ => false,
    }
}

/// A receiver-less, argument-less call to `private`/`protected`/`public`
/// (RuboCop's inline `(send nil? {:private :protected :public})` pattern --
/// deliberately narrower than the general `bare_access_modifier?` helper,
/// which also allows `module_function`).
fn is_bare_access_modifier(node: &Node<'_>) -> bool {
    let call = node.as_call_node().expect("kind matched");
    if call.receiver().is_some() || call.arguments().is_some() {
        return false;
    }
    matches!(call.name().as_slice(), b"private" | b"protected" | b"public")
}

/// RuboCop's `node.type` as used by `deferred_message` -- whitequark's
/// `:def`/`:defs` split on whether the method has an explicit receiver
/// (`def self.foo`), plus `:class`/`:module`/`:send`.
fn node_type_name(node: &Node<'_>) -> &'static str {
    match node {
        Node::DefNode { .. } => {
            let def = node.as_def_node().expect("kind matched");
            if def.receiver().is_some() {
                "defs"
            } else {
                "def"
            }
        }
        Node::ClassNode { .. } => "class",
        Node::ModuleNode { .. } => "module",
        _ => "send",
    }
}

/// RuboCop's `check_both`.
pub(crate) fn check_both(
    ctx: &mut Context<'_>,
    meta: &'static RuleMeta,
    kind: &str,
    want: Want,
    first_line: u32,
    last_line: u32,
) {
    check_beginning(ctx, meta, kind, want, first_line);
    check_ending(ctx, meta, kind, want, last_line);
}

/// RuboCop's `check_beginning`/`check_source`/`check_line` for the body's
/// first line: the target line is the one right after the construct's own
/// first line (possibly adjusted by the caller, e.g. for a multi-line
/// argument list or superclass expression).
pub(crate) fn check_beginning(
    ctx: &mut Context<'_>,
    meta: &'static RuleMeta,
    kind: &str,
    want: Want,
    first_line: u32,
) {
    let target = first_line + 1;
    let blank = ctx.line_text(target).is_empty();
    match want {
        Want::NoEmpty if blank => report_extra(ctx, meta, kind, target, "beginning"),
        Want::Empty if !blank => report_missing(ctx, meta, kind, target, "beginning"),
        Want::NoEmpty | Want::Empty => {}
    }
}

/// RuboCop's `check_ending`/`check_source`/`check_line` for the body's last
/// line: the target line checked is the one right before `end`, but a
/// missing-blank-line offense is reported on the `end` line itself
/// (RuboCop's `check_line` offset quirk: `offset = 2` when the message
/// mentions `'end.'`).
pub(crate) fn check_ending(
    ctx: &mut Context<'_>,
    meta: &'static RuleMeta,
    kind: &str,
    want: Want,
    last_line: u32,
) {
    let target = last_line - 1;
    let blank = ctx.line_text(target).is_empty();
    match want {
        Want::NoEmpty if blank => report_extra(ctx, meta, kind, target, "end"),
        Want::Empty if !blank => report_missing(ctx, meta, kind, last_line, "end"),
        Want::NoEmpty | Want::Empty => {}
    }
}

/// RuboCop's `check_deferred_empty_line`: when the body's first statement
/// doesn't itself require a leading blank line (a bare `def`/`class`/
/// `module`/access-modifier), but a *later* sibling does, a blank line is
/// still required directly before that sibling (skipping back over any
/// contiguous full-line comments), unless one is already there.
pub(crate) fn check_deferred_empty_line(
    ctx: &mut Context<'_>,
    meta: &'static RuleMeta,
    shape: &BodyShape<'_>,
) {
    let Some(child) = first_empty_line_required_child(shape) else { return };
    let child_first_line = ctx.line_col(child.location().span().start).line;
    let prev_line = previous_line_ignoring_comments(ctx, child_first_line);
    if ctx.line_text(prev_line).is_empty() {
        return;
    }
    let target = prev_line + 1;
    let span = char_span(ctx, target);
    let msg = format!("Empty line missing before first {} definition", node_type_name(child));
    let fix = Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::insert(span.start, b"\n".as_slice())],
    };
    ctx.report_with_fix(meta, span, msg, fix);
}

/// RuboCop's `previous_line_ignoring_comments`: the closest line at or
/// before `send_line - 1` that isn't a full-line comment, or line 1 if
/// every line up to the top of the file is a comment.
pub(crate) fn previous_line_ignoring_comments(ctx: &Context<'_>, send_line: u32) -> u32 {
    for candidate in (1..send_line.max(1)).rev() {
        if !ruby_source::is_comment_line(ctx.line_text(candidate)) {
            return candidate;
        }
    }
    1
}

/// An "Extra empty line detected" offense: `range` is the blank line's own
/// newline byte (a real 1-byte span whose end offset lands on the next
/// physical line, rendering as a zero-width caret), deleted to collapse the
/// blank line away.
fn report_extra(ctx: &mut Context<'_>, meta: &'static RuleMeta, kind: &str, line: u32, desc: &str) {
    let span = char_span(ctx, line);
    let msg = format!("Extra empty line detected at {kind} body {desc}.");
    let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] };
    ctx.report_with_fix(meta, span, msg, fix);
}

/// An "Empty line missing" offense: `range` is the target line's own first
/// character, with a newline inserted before it to add the required blank
/// line.
fn report_missing(
    ctx: &mut Context<'_>,
    meta: &'static RuleMeta,
    kind: &str,
    line: u32,
    desc: &str,
) {
    let span = char_span(ctx, line);
    let msg = format!("Empty line missing at {kind} body {desc}.");
    let fix = Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::insert(span.start, b"\n".as_slice())],
    };
    ctx.report_with_fix(meta, span, msg, fix);
}

/// A real 1-byte span at the start of `line`, matching `Layout::EmptyLines`'
/// convention for a diagnostic-plus-edit range anchored on one line.
pub(crate) fn char_span(ctx: &Context<'_>, line: u32) -> Span {
    let start = ctx.line_span(line).start;
    Span::new(start, start + 1)
}
