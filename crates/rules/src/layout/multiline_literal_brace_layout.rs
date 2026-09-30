//! Shared logic behind RuboCop's `MultilineLiteralBraceLayout` mixin
//! (`lib/rubocop/cop/mixin/multiline_literal_brace_layout.rb`),
//! `MultilineLiteralBraceCorrector`
//! (`lib/rubocop/cop/correctors/multiline_literal_brace_corrector.rb`), and
//! the `ConfigurableEnforcedStyle`'s `EnforcedStyle` option they both read --
//! used by `Layout/MultilineArrayBraceLayout`, `Layout/MultilineHashBraceLayout`,
//! `Layout/MultilineMethodCallBraceLayout`, and
//! `Layout/MultilineMethodDefinitionBraceLayout`.
//!
//! Each cop hands [`check_brace_layout`] a [`BraceLiteral`) describing its own
//! notion of "children" (RuboCop's overridable `children(node)`):
//! `ArrayNode`/`HashNode`'s elements, a `CallNode`'s arguments, or a `def`'s
//! flattened parameter list. [`BraceLiteral::opening`]/`closing` are the
//! literal's own delimiter spans (`None` opening means an implicit literal --
//! RuboCop's `!node.loc.begin` -- always skipped). `whole_span` is the span
//! RuboCop's `node.single_line?` guard reads: for the array/hash/def cops
//! this is exactly `opening`..`closing`, but `Layout/MultilineMethodCallBraceLayout`
//! passes the *whole call's* span (receiver included), reproducing its
//! `single_line_ignoring_receiver?` override, which that cop's own `enter`
//! additionally short-circuits before even building a [`BraceLiteral`] (see
//! its module doc comment).
//!
//! RuboCop's `node.chained?`/`node.argument?` (used only to suppress
//! autocorrection when a trailing comment blocks a safe edit, never to
//! suppress detection) depend on the literal's own parent, which the
//! engine's single top-down pass has not yet examined when it visits a
//! child. [`record_call_relations`] runs when visiting a `CallNode` in
//! parent position -- before the engine descends into that call's receiver
//! or arguments -- and records both relations by span into the caller's own
//! per-file `HashSet`s, looked up when the engine later reaches the spans
//! themselves. `Layout/MultilineMethodDefinitionBraceLayout` never sets
//! `chained_or_argument` at all: its "node" (the parameter list) is always
//! parented by the `def` itself, which is neither a call nor ever
//! `send_type?`.

use std::collections::HashSet;

use linter::{Applicability, Context, Edit, Fix, OptionError, RuleMeta, RuleOptions};
use ruby_ast::{for_each_child, node::CallNode, LocationExt as _, Node, NodeExt as _};
use ruby_source::{Side, Span};

/// RuboCop's `ConfigurableEnforcedStyle`'s `EnforcedStyle` for this mixin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Style {
    Symmetrical,
    NewLine,
    SameLine,
}

/// Parses the shared `EnforcedStyle` option every cop in this family uses.
pub(crate) fn resolve_style(options: &RuleOptions) -> Result<Style, OptionError> {
    Ok(match options.style("EnforcedStyle")? {
        "new_line" => Style::NewLine,
        "same_line" => Style::SameLine,
        _ => Style::Symmetrical,
    })
}

/// One cop's four `MSG`-alike constants.
pub(crate) struct Messages {
    pub same: &'static str,
    pub new: &'static str,
    pub always_new: &'static str,
    pub always_same: &'static str,
}

/// Everything [`check_brace_layout`] needs about one literal/call/parameter
/// list, in RuboCop's `MultilineLiteralBraceLayout` terms. See the module
/// doc comment for what each field maps to per cop.
pub(crate) struct BraceLiteral<'pr> {
    /// RuboCop's `node.loc.begin`; `None` means an implicit literal.
    pub opening: Option<Span>,
    /// RuboCop's `node.loc.end`.
    pub closing: Span,
    /// RuboCop's (possibly overridden) `children(node)`.
    pub children: Vec<Node<'pr>>,
    /// RuboCop's `node.single_line?` span (see the module doc comment).
    pub whole_span: Span,
    /// RuboCop's `node.chained? || node.argument?`.
    pub chained_or_argument: bool,
    /// RuboCop's `use_heredoc_argument_method_chain?`: the enclosing chained
    /// call's `.`/`&.` operator through its own end (to delete), and that
    /// same text (to reinsert right after the fixed-up closing delimiter).
    /// Only ever `Some` for `Layout/MultilineMethodCallBraceLayout`.
    pub heredoc_chain: Option<(Span, Box<[u8]>)>,
}

/// RuboCop's `check_brace_layout`, `check`, `check_new_line`,
/// `check_same_line`, and `check_symmetrical`.
pub(crate) fn check_brace_layout(
    ctx: &mut Context<'_>,
    meta: &RuleMeta,
    style: Style,
    messages: &Messages,
    literal: &BraceLiteral<'_>,
) {
    // `ignored_literal?`: `implicit_literal?` / `empty_literal?` / `single_line?`.
    let Some(opening) = literal.opening else { return };
    let Some(&last) = literal.children.last() else { return };
    if ctx.is_single_line(literal.whole_span) {
        return;
    }
    // `last_line_heredoc?(node.children.last)`.
    if last_line_heredoc(ctx, &last, ctx.last_line(last.span())) {
        return;
    }

    let first = literal.children.first().expect("checked non-empty above");
    let opening_same_line = ctx.same_line(opening, first.span());
    let closing_same_line = ctx.line_col(literal.closing.start).line == ctx.last_line(last.span());

    let message = match style {
        Style::Symmetrical if opening_same_line => {
            if closing_same_line {
                return;
            }
            messages.same
        }
        Style::Symmetrical => {
            if !closing_same_line {
                return;
            }
            messages.new
        }
        Style::NewLine => {
            if !closing_same_line {
                return;
            }
            messages.always_new
        }
        Style::SameLine => {
            if closing_same_line {
                return;
            }
            messages.always_same
        }
    };

    match build_correction(ctx, literal, &last, closing_same_line) {
        Some(fix) => ctx.report_with_fix(meta, literal.closing, message, fix),
        None => ctx.report(meta, literal.closing, message),
    }
}

/// RuboCop's `MultilineLiteralBraceCorrector#call` plus
/// `correct_same_line_brace`/`correct_next_line_brace`/
/// `correct_heredoc_argument_method_chain`/`content_if_comment_present`.
/// `None` means "report without a fix" (RuboCop's
/// `new_line_needed_before_closing_brace?` guard: a comment before the
/// closing delimiter of a chained/argument literal blocks a safe edit).
fn build_correction(
    ctx: &Context<'_>,
    literal: &BraceLiteral<'_>,
    last: &Node<'_>,
    closing_same_line: bool,
) -> Option<Fix> {
    let mut edits = Vec::new();
    if closing_same_line {
        // `correct_same_line_brace`.
        edits.push(Edit::insert(literal.closing.start, b"\n".as_slice()));
    } else {
        // `new_line_needed_before_closing_brace?`.
        let last_line = ctx.last_line(last.span());
        let last_line_commented = ctx.comments().iter().any(|c| c.line == last_line);
        if literal.chained_or_argument && last_line_commented {
            return None;
        }

        // `last_element_range_with_trailing_comma(node).end`: the trailing
        // comma, when present, always immediately follows the last child
        // (RuboCop's own trailing-comma probe never crosses a gap either).
        let child_end = last.span().end;
        let end_point = if ctx.text(Span::new(child_end, child_end + 1)) == b"," {
            child_end + 1
        } else {
            child_end
        };

        // `correct_next_line_brace`: `remove(range_with_surrounding_space(node.loc.end,
        // side: :left))` plus, when a comment sits on the last child's own line,
        // `content_if_comment_present`'s wider capture-and-remove -- merged into one
        // non-overlapping deletion (this engine's `Fix::edits` may not overlap).
        let base_remove = ctx.with_surrounding_space(literal.closing, Side::Left, true, false);
        let (remove_span, mut insert_text): (Span, Vec<u8>) = if last_line_commented {
            // `range_by_whole_lines(node.source_range)`'s default
            // `include_final_newline: false` -- `ctx.line_span` (not
            // `ctx.whole_lines`, which always includes the trailing
            // newline) is the matching primitive.
            let whole_line = ctx.line_span(ctx.line_col(literal.closing.start).line);
            let captured = Span::new(literal.closing.start, whole_line.end);
            (Span::new(base_remove.start, whole_line.end), ctx.text(captured).to_vec())
        } else {
            (base_remove, ctx.text(literal.closing).to_vec())
        };
        edits.push(Edit::delete(remove_span));

        // `correct_heredoc_argument_method_chain`.
        if let Some((chain_remove, chain_text)) = &literal.heredoc_chain {
            edits.push(Edit::delete(*chain_remove));
            insert_text.extend_from_slice(chain_text);
        }
        edits.push(Edit::insert(end_point, insert_text));
    }
    Some(Fix { applicability: Applicability::Safe, edits })
}

/// RuboCop's `last_line_heredoc?`: true when `node` is, or contains, a
/// heredoc whose closing delimiter's own line is on or after `boundary` --
/// unsafe to edit around, since the fix could interleave with heredoc
/// content. `boundary` is the top-level node's own last line, computed once
/// by the caller (Prism's heredoc node span, like whitequark's, stops at the
/// opener, so a bare heredoc's own `last_line` is always its *opening* line).
fn last_line_heredoc(ctx: &Context<'_>, node: &Node<'_>, boundary: u32) -> bool {
    if let Some(closing) = heredoc_closing(ctx, node) {
        if ctx.last_line(closing) >= boundary {
            return true;
        }
    }
    let mut found = false;
    for_each_child(node, |child| {
        found = found || last_line_heredoc(ctx, child, boundary);
    });
    found
}

/// The closing delimiter's span for a heredoc string/xstring literal (see
/// [`linter::heredoc_bodies`], whose opening/closing detection this mirrors).
fn heredoc_closing(ctx: &Context<'_>, node: &Node<'_>) -> Option<Span> {
    let opening_closing = match node {
        Node::StringNode { .. } => {
            let n = node.as_string_node().expect("kind matched");
            n.opening_loc().zip(n.closing_loc())
        }
        Node::InterpolatedStringNode { .. } => {
            let n = node.as_interpolated_string_node().expect("kind matched");
            n.opening_loc().zip(n.closing_loc())
        }
        Node::XStringNode { .. } => {
            let n = node.as_x_string_node().expect("kind matched");
            Some((n.opening_loc(), n.closing_loc()))
        }
        Node::InterpolatedXStringNode { .. } => {
            let n = node.as_interpolated_x_string_node().expect("kind matched");
            Some((n.opening_loc(), n.closing_loc()))
        }
        _ => None,
    };
    let (open, close) = opening_closing?;
    ctx.text(open.span()).starts_with(b"<<").then(|| close.span())
}

/// RuboCop's `chained?`/`argument?`, precomputed while visiting a `CallNode`
/// in parent position (see the module doc comment).
pub(crate) fn record_call_relations(
    call: &CallNode<'_>,
    chained: &mut HashSet<Span>,
    arguments: &mut HashSet<Span>,
) {
    if let Some(receiver) = call.receiver() {
        chained.insert(receiver.span());
    }
    // `argument?`'s `parent&.send_type?` excludes a safe-navigation parent.
    if !call.is_safe_navigation() {
        if let Some(args) = call.arguments() {
            for arg in &args.arguments() {
                arguments.insert(arg.span());
            }
        }
    }
}
