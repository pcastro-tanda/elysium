//! `Style/IfInsideElse`, ported from RuboCop's
//! `lib/rubocop/cop/style/if_inside_else.rb`.
//!
//! # Node shapes
//!
//! Whitequark represents a genuine terminal `else` clause as the raw content
//! of the `if` sexp's third child directly -- no wrapper node -- so a lone
//! `if` statement sitting there IS that child, elided the same way a single
//! statement anywhere else is. Prism instead always wraps a real terminal
//! `else` in an [`ruby_ast::node::ElseNode`], whose own
//! [`ruby_ast::node::ElseNode::statements`] holds the body.
//! [`nested_if_in_else`] reconstructs the whitequark shape: `outer`'s
//! [`IfNode::subsequent`] must be `Some(ElseNode)` (an `elsif` continuation
//! is instead `Some(IfNode)`, excluded automatically since it is never
//! unwrapped here), whose body is exactly one statement that is itself an
//! `IfNode`.
//!
//! `else_branch.if_type? && else_branch.if?` (upstream) becomes: the
//! candidate's own [`IfNode::if_keyword_loc`] must be present (ternaries
//! share the `IfNode` kind but carry none) and read `"if"`, never `"elsif"`.
//! The latter can only actually occur by reaching a chain link through
//! [`IfNode::subsequent`], which this module never does when unwrapping an
//! `ElseNode`'s body -- so the check is dead code here, kept only for
//! fidelity with upstream's guard.
//!
//! A ternary as `outer` itself (`ignores_nested_ternary_expressions`) is
//! excluded the same way: `outer.if_keyword_loc().is_none()`.
//!
//! `node.modifier_form?` is rubocop-ast's `(if? || unless?) && loc.end.nil?`;
//! since `outer`/the candidate are already known to carry an `if` keyword,
//! this is just [`IfNode::end_keyword_loc`] being `None`.
//!
//! # The `then`-form detour
//!
//! When the candidate has its own `then` keyword (`if b then foo end`),
//! upstream's `autocorrect` delegates entirely to `IfThenCorrector`
//! (replacing the *whole* candidate with a reconstructed multiline `if`,
//! `elsif`-by-`elsif`, ending at a bare `end` or a reconstructed `else`) and
//! returns *without* touching `outer`'s own `else` keyword at all --
//! [`build_then_fix`] mirrors that reconstruction. The `elsif`/to-`elsif`
//! conversion this cop is actually named for only happens once a later fix
//! round re-lints the now-`then`-free source and matches again from
//! scratch. `crates/linter`'s `fix_file` already reruns rules to a fixed
//! point with fresh rule state each round (matching a real
//! `rubocop -A` invocation, confirmed directly against 1.91.0's CLI): no
//! `ignore_node`-style bookkeeping is reproduced here, and none is needed --
//! two structurally independent matches (an outer conversion and a nested
//! one several levels down) always land their edits on disjoint lines (the
//! `outer`'s own `else` line, the candidate's own header line, and the
//! candidate's own `end` line; never anything in between, which is exactly
//! where a deeper nested match's edits live), so a single round's
//! non-overlapping-edit application already accepts every independent match
//! at once, and any leftover `then`-form conversion converges on the very
//! next round. See `crates/rules/fixtures/README.md` for the four upstream
//! spec examples that print a *partial* `expect_correction` (stuck
//! mid-conversion) purely because `RSpec`'s `expect_correction` reuses one
//! cop instance -- and its `@ignored_nodes` -- across its own internal retry
//! loop; verified directly against `rubocop 1.91.0 -A` that real usage
//! converges the rest of the way on every one of them, so those four
//! fixture directories were deleted rather than pinning the `RSpec` artifact.
//!
//! # `range_with_comments`
//!
//! The plain (non-`then`, non-modifier) conversion moves the candidate's
//! own `if`/condition header out and, in its place, splices in the
//! candidate's *if-branch* text -- extended, like upstream's
//! `range_with_comments`, over a contiguous same-line-adjacent leading
//! comment block and a trailing same-line comment -- then deletes that
//! text's original location as whole lines. [`extend_with_comments`]
//! reconstructs that extension directly (no `ast_with_comments` map exists
//! here).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ElseNode, IfNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Convert `if` nested inside `else` to `elsif`.";

/// Finds if nodes inside else, which can be converted to elsif.
#[derive(Debug, Clone)]
pub struct IfInsideElse {
    /// `AllowIfModifier`. Absent from `config/default.yml` upstream (so
    /// `cop_config['AllowIfModifier']` reads `nil`, falsy); every spec
    /// example other than the explicit `AllowIfModifier is true` context
    /// sets it to `false` outright, so `false` is the practical default.
    allow_if_modifier: bool,
}

impl Rule for IfInsideElse {
    const META: RuleMeta = RuleMeta {
        name: "Style/IfInsideElse",
        department: Department::Style,
        summary: "Finds if nodes inside else, which can be converted to elsif.",
        explanation: "\
If the `else` branch of a conditional consists solely of an `if` node, it can \
be combined with the `else` to become an `elsif`. This helps to keep the \
nesting level from getting too deep.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode],
        config: &[ConfigOption {
            name: "AllowIfModifier",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Allow a modifier-form `if` (`foo if bar`) as the sole content of an `else`.",
        }],
        blind_spots: "\
`range_with_comments`'s real comment association (`ast_with_comments`) is
reconstructed as: a contiguous run of comment-only lines directly above the
if-branch (no blank-line gap), plus a single trailing same-line comment
after it. A comment separated from the branch by a blank line, or floating
inside a multi-statement branch, is not folded in and is instead silently
dropped by the surrounding whole-line deletion -- unobserved in the corpus
so far.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_if_modifier: options.bool("AllowIfModifier") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(outer) = node.as_if_node() else { return };
        if outer.if_keyword_loc().is_none() {
            return; // Ternary: no `if`/`else` keyword shape at all.
        }
        let Some(candidate) = nested_if_in_else(&outer) else { return };
        let Some(candidate_kw) = candidate.if_keyword_loc() else { return }; // Ternary.
        if ctx.text(candidate_kw.span()) != b"if" {
            return; // `elsif`: unreachable here, see module doc.
        }
        let is_modifier = candidate.end_keyword_loc().is_none();
        if self.allow_if_modifier && is_modifier {
            return;
        }
        let else_node = outer
            .subsequent()
            .and_then(|s| s.as_else_node())
            .expect("nested_if_in_else only matches through a real `ElseNode`");
        if !is_modifier {
            let else_kw = else_node.else_keyword_loc();
            if comment_between(ctx, else_kw.span().end, candidate_kw.span().start) {
                return;
            }
        }
        let fix = if candidate.then_keyword_loc().is_some() {
            build_then_fix(ctx, &candidate)
        } else if is_modifier {
            build_modifier_fix(ctx, &else_node, &candidate)
        } else {
            build_plain_fix(ctx, &else_node, &candidate, candidate_kw.span())
        };
        ctx.report_with_fix(&Self::META, candidate_kw.span(), MSG, fix);
    }
}

/// RuboCop's `else_branch`, narrowed to the shape this cop cares about: a
/// genuine terminal `else` (`outer.subsequent()` is an [`ElseNode`], never
/// an `elsif` continuation) whose body is exactly one statement, itself an
/// `if`. Returns `None` for zero or more-than-one statements (whitequark's
/// `nil`/`begin` cases, neither of which is `if_type?`) same as an absent or
/// non-`if` sole statement.
fn nested_if_in_else<'pr>(outer: &IfNode<'pr>) -> Option<IfNode<'pr>> {
    let else_node = outer.subsequent()?.as_else_node()?;
    let statements = else_node.statements()?;
    let mut body = statements.body().iter();
    match (body.next(), body.next()) {
        (Some(only), None) => only.as_if_node(),
        _ => None,
    }
}

/// RuboCop's `comments_between_else_and_if?`, always called with
/// `else_branch.modifier_form?` already false (the caller skips this check
/// entirely for a modifier candidate, matching upstream's early `return
/// false`).
fn comment_between(ctx: &Context<'_>, else_end: u32, if_begin: u32) -> bool {
    ctx.comments().iter().any(|c| c.span.start > else_end && c.span.start < if_begin)
}

/// RuboCop's `IfThenCorrector`, called with a constant `indentation: 0` (so
/// its `branch_body_indentation` is always empty): reconstructs `node` as a
/// multiline `if`/`elsif`.../`else`/`end`, replacing its own `then` form
/// entirely. `indentation` is the fixed column width computed once, up
/// front, from `node`'s own start column, then threaded unchanged through
/// every recursive `elsif` link.
fn then_form_replacement(ctx: &Context<'_>, node: &IfNode<'_>, indentation: &[u8]) -> Vec<u8> {
    let keyword_loc = node.if_keyword_loc().expect("if/elsif keyword present");
    let keyword = ctx.text(keyword_loc.span());
    let is_elsif = keyword == b"elsif";
    let condition = ctx.text(node.predicate().span());
    let branch_source: &[u8] =
        node.statements().map_or(&b"nil"[..], |s| ctx.text(s.location().span()));

    let mut out = Vec::new();
    if is_elsif {
        out.extend_from_slice(indentation);
    }
    out.extend_from_slice(keyword);
    out.push(b' ');
    out.extend_from_slice(condition);
    out.push(b'\n');
    out.extend_from_slice(indentation);
    out.extend_from_slice(branch_source);
    out.push(b'\n');
    match node.subsequent() {
        None => out.extend_from_slice(b"end"),
        Some(next) => {
            if let Some(elsif_next) = next.as_if_node() {
                out.extend(then_form_replacement(ctx, &elsif_next, indentation));
            } else {
                let else_node = next.as_else_node().expect("subsequent is `if` or `else`");
                let else_source: &[u8] =
                    else_node.statements().map_or(&b""[..], |s| ctx.text(s.location().span()));
                out.extend_from_slice(indentation);
                out.extend_from_slice(b"else\n");
                out.extend_from_slice(indentation);
                out.extend_from_slice(else_source);
                out.push(b'\n');
                out.extend_from_slice(indentation);
                out.extend_from_slice(b"end");
            }
        }
    }
    out
}

/// The `node.then?` branch of `autocorrect`: replaces `candidate` wholesale,
/// never touching `outer`'s own `else` keyword (see the module doc for why
/// that conversion is left to a later fix round).
fn build_then_fix(ctx: &Context<'_>, candidate: &IfNode<'_>) -> Fix {
    let span = candidate.location().span();
    let column = ctx.line_col(span.start).column;
    let indentation = vec![b' '; column as usize];
    let replacement = then_form_replacement(ctx, candidate, &indentation);
    Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(span, replacement)] }
}

/// `correct_to_elsif_from_modifier_form`: `outer`'s `else` becomes `elsif
/// <condition>`, and the trailing ` if <condition>` is deleted off the
/// modifier statement, leaving its own body in place.
fn build_modifier_fix(ctx: &Context<'_>, else_node: &ElseNode<'_>, candidate: &IfNode<'_>) -> Fix {
    let condition_span = candidate.predicate().span();
    let body_end =
        candidate.statements().expect("a modifier `if` always has a body").location().span().end;
    let mut replacement = b"elsif ".to_vec();
    replacement.extend_from_slice(ctx.text(condition_span));
    Fix {
        applicability: Applicability::Safe,
        edits: vec![
            Edit::replace(else_node.else_keyword_loc().span(), replacement),
            Edit::delete(Span::new(body_end, condition_span.end)),
        ],
    }
}

/// `correct_to_elsif_from_if_inside_else_form`: `outer`'s `else` becomes
/// `elsif <condition>`; `candidate`'s own header (`if <condition>`) is
/// replaced by its if-branch's text (extended over adjacent comments, see
/// the module doc), whose original location is then deleted -- or, with no
/// if-branch at all, the header line is simply deleted outright; either way
/// `candidate`'s own `end` line goes too, leaving its `else`/body (if any)
/// standing as `outer`'s new real `else` clause.
fn build_plain_fix(
    ctx: &Context<'_>,
    else_node: &ElseNode<'_>,
    candidate: &IfNode<'_>,
    candidate_kw: Span,
) -> Fix {
    let condition_span = candidate.predicate().span();
    let header_span = Span::new(candidate_kw.start, condition_span.end);
    let mut replacement = b"elsif ".to_vec();
    replacement.extend_from_slice(ctx.text(condition_span));
    let mut edits = vec![Edit::replace(else_node.else_keyword_loc().span(), replacement)];
    match candidate.statements() {
        Some(branch) => {
            let extended = extend_with_comments(ctx, branch.location().span(), header_span.end);
            edits.push(Edit::replace(header_span, ctx.text(extended).to_vec()));
            edits.push(Edit::delete(ctx.whole_lines(extended)));
        }
        None => edits.push(Edit::delete(ctx.whole_lines(header_span))),
    }
    let end_span = candidate
        .end_keyword_loc()
        .expect("non-modifier, non-`then` `if` always has its own `end`")
        .span();
    edits.push(Edit::delete(ctx.whole_lines(end_span)));
    Fix { applicability: Applicability::Safe, edits }
}

/// RuboCop's `range_with_comments(if_branch)`, narrowed to what an
/// `ast_with_comments` association ever contributes here: a single trailing
/// same-line comment, and a contiguous run of comment-only lines directly
/// above `span` with no blank-line gap (never crossing back past
/// `lower_bound`, the end of `candidate`'s own header).
fn extend_with_comments(ctx: &Context<'_>, mut span: Span, lower_bound: u32) -> Span {
    let comments = ctx.comments();
    let last_line = ctx.line_col(span.end.saturating_sub(1).max(span.start)).line;
    if let Some(c) = comments
        .iter()
        .find(|c| c.span.start >= span.end && ctx.line_col(c.span.start).line == last_line)
    {
        span.end = span.end.max(c.span.end);
    }
    loop {
        let cur_line = ctx.line_col(span.start).line;
        if cur_line <= 1 {
            break;
        }
        let found = comments.iter().rev().find(|c| {
            c.span.end <= span.start
                && c.span.start >= lower_bound
                && ctx.line_col(c.span.start).line == cur_line - 1
                && ctx.line_col(c.span.end.saturating_sub(1)).line == cur_line - 1
        });
        match found {
            Some(c) => span.start = c.span.start,
            None => break,
        }
    }
    span
}
