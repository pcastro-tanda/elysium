//! Shared plumbing for RuboCop's `Alignment` mixin
//! (`lib/rubocop/cop/mixin/alignment.rb`) plus `AlignmentCorrector`
//! (`lib/rubocop/cop/correctors/alignment_corrector.rb`), used by
//! `Layout/ArrayAlignment` and `Layout/ParameterAlignment`.
//! `Layout/ArgumentAlignment` keeps its own copy of this same logic since it
//! additionally has to flatten a braceless keyword hash's pairs into the
//! item list before checking alignment.

use linter::{Applicability, Context, Fix, RuleMeta};
use ruby_ast::{Node, NodeExt as _};
use ruby_source::Span;

/// RuboCop's `/\S.*/.match(line).begin(0)`: the byte column of the first
/// non-whitespace character on `line`, or `0` for a blank line.
pub(crate) fn indentation_of_line(ctx: &Context<'_>, line: u32) -> u32 {
    let text = ctx.line_text(line);
    text.iter()
        .position(|&b| !b.is_ascii_whitespace())
        .map_or(0, |pos| u32::try_from(pos).unwrap_or(u32::MAX))
}

/// RuboCop's `AlignmentCorrector.correct`: shifts every physical line of
/// `item` by `column_delta` columns. Returns `None` when nothing could be
/// safely edited (a `=begin`/`=end` block comment inside the range, or every
/// line was blocked by a taboo range/whitespace mismatch).
fn build_fix(ctx: &Context<'_>, item: &Node<'_>, column_delta: i64) -> Option<Fix> {
    let span = item.span();
    let taboo = linter::heredoc_bodies(ctx, item);
    let delta = i32::try_from(column_delta).unwrap_or(0);
    let edits = linter::shift_lines(ctx, span, delta, &taboo);
    if edits.is_empty() {
        None
    } else {
        Some(Fix { applicability: Applicability::Safe, edits })
    }
}

/// RuboCop's `Alignment#check_alignment` + `#each_bad_alignment` +
/// `#register_offense`. `reported` accumulates every span already reported
/// this file (RuboCop's `@current_offenses`, scoped per cop instance here
/// instead of per commissioner run): an offense nested within a previously
/// reported one still reports (the `within?` branch) but is not separately
/// autocorrected, since two rewrites in the same area cannot be handled in
/// one pass.
pub(crate) fn check_alignment(
    ctx: &mut Context<'_>,
    meta: &RuleMeta,
    items: &[Node<'_>],
    base_column: i64,
    message: &'static str,
    reported: &mut Vec<Span>,
) {
    let mut prev_line: i64 = -1;
    for item in items {
        let span = item.span();
        let line = i64::from(ctx.line_col(span.start).line);
        if line > prev_line && ctx.begins_its_line(span) {
            let column_delta = base_column - i64::from(ctx.display_column(span.start));
            if column_delta != 0 {
                let nested = reported.iter().any(|reported| reported.contains(span));
                reported.push(span);
                if nested {
                    ctx.report(meta, span, message);
                } else {
                    match build_fix(ctx, item, column_delta) {
                        Some(fix) => ctx.report_with_fix(meta, span, message, fix),
                        None => ctx.report(meta, span, message),
                    }
                }
            }
        }
        prev_line = line;
    }
}
