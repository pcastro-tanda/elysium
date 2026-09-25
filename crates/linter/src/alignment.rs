//! RuboCop's `AlignmentCorrector`: the shared multiline shift-edit builder
//! behind every alignment/indentation autocorrection (`Layout/ArgumentAlignment`,
//! `Layout/FirstArgumentIndentation`, `Layout/FirstHashElementIndentation`,
//! `Layout/IndentationConsistency`, `Layout/IndentationWidth`,
//! `Style/ClassAndModuleChildren`).

use ruby_ast::{walk, LocationExt as _, Node, Visitor};
use ruby_source::Span;

use crate::{Context, Edit};

/// RuboCop-AST's `AlignmentCorrector#inside_string_ranges`'s heredoc case:
/// the byte span of every heredoc body within `node`, so [`shift_lines`]
/// never touches heredoc content when it walks `node`'s lines. Pass the
/// result as `taboo`.
#[must_use]
pub fn heredoc_bodies(ctx: &Context<'_>, node: &Node<'_>) -> Vec<Span> {
    struct Finder<'ctx, 'src> {
        ctx: &'ctx Context<'src>,
        ranges: Vec<Span>,
    }

    impl<'pr> Visitor<'pr> for Finder<'_, '_> {
        fn enter(&mut self, node: &Node<'pr>) {
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
            if let Some((open, close)) = opening_closing {
                if self.ctx.text(open.span()).starts_with(b"<<") {
                    self.ranges.push(Span::new(open.span().end, close.span().start));
                }
            }
        }
    }

    let mut finder = Finder { ctx, ranges: Vec::new() };
    walk(node, &mut finder);
    finder.ranges
}

/// RuboCop's `AlignmentCorrector.correct`: shifts every physical line of
/// `span` by `column_delta` columns, inserting or deleting leading
/// whitespace. On the first line the shift is anchored at `span.start`
/// itself (mid-line, for a node that does not begin its own line); every
/// other line is anchored at its own start. A line is left untouched when
/// indenting a blank line, when the insertion point or deletion range falls
/// inside a `taboo` span (heredoc bodies, `=begin`/`=end` blocks -- see
/// [`heredoc_bodies`]), or when a dedent's target bytes are not all
/// spaces/tabs. Returns no edits at all when a `=begin`/`=end` block
/// comment's opener intersects `span`, since correcting around it would
/// otherwise re-indent the block comment itself.
#[must_use]
pub fn shift_lines(ctx: &Context<'_>, span: Span, column_delta: i32, taboo: &[Span]) -> Vec<Edit> {
    if column_delta == 0 {
        return Vec::new();
    }
    let start_line = ctx.line_col(span.start).line;
    let last_byte = span.end.saturating_sub(1).max(span.start);
    let end_line = ctx.line_col(last_byte).line;

    for line in start_line..=end_line {
        if ctx.line_text(line).trim_ascii_start().starts_with(b"=begin") {
            return Vec::new();
        }
    }

    let amount = column_delta.unsigned_abs();
    let mut edits = Vec::new();
    for line in start_line..=end_line {
        let is_first = line == start_line;
        let anchor = if is_first { span.start } else { ctx.line_span(line).start };

        if column_delta > 0 {
            if !is_first && ctx.line_span(line).is_empty() {
                continue;
            }
            if taboo.iter().any(|t| t.contains(Span::empty(anchor))) {
                continue;
            }
            edits.push(Edit::insert(anchor, " ".repeat(amount as usize).into_bytes()));
        } else {
            let starts_with_space =
                ctx.source().bytes().get(anchor as usize).is_some_and(|&b| b == b' ');
            let range = if is_first || !starts_with_space {
                Span::new(anchor.saturating_sub(amount), anchor)
            } else {
                Span::new(anchor, anchor + amount)
            };
            if taboo.iter().any(|t| t.contains(range)) {
                continue;
            }
            let text = ctx.text(range);
            if !text.is_empty() && text.iter().all(|&b| b == b' ' || b == b'\t') {
                edits.push(Edit::delete(range));
            }
        }
    }
    edits
}

#[cfg(test)]
mod tests {
    use ruby_ast::{NodeExt as _, Parsed};
    use ruby_directives::Directives;
    use ruby_source::SourceFile;

    use super::*;

    fn ctx_for<'a>(source: &'a SourceFile, parsed: &'a Parsed<'a>) -> Context<'a> {
        Context::new(source, parsed, Directives::default())
    }

    #[test]
    fn indent_anchors_first_line_at_span_start_and_skips_blank_lines() {
        // Span starts mid-line-1 (at "abc", offset 0) and runs through line 4;
        // line 3 is blank and must not get an inserted edit.
        let text = b"abc\r\n  def\r\n\r\n  ghi\r\n".to_vec();
        let source = SourceFile::new("a.rb", text.clone());
        let parsed = Parsed::parse(&source);
        let ctx = ctx_for(&source, &parsed);
        let span = Span::new(0, u32::try_from(text.len()).unwrap());

        let edits = shift_lines(&ctx, span, 2, &[]);

        let def_start = u32::try_from(text.windows(3).position(|w| w == b"  d").unwrap()).unwrap();
        let ghi_start = u32::try_from(text.windows(3).position(|w| w == b"  g").unwrap()).unwrap();
        assert_eq!(
            edits,
            vec![
                Edit::insert(0, b"  ".to_vec()),
                Edit::insert(def_start, b"  ".to_vec()),
                Edit::insert(ghi_start, b"  ".to_vec()),
            ]
        );
    }

    #[test]
    fn dedent_deletes_leading_whitespace_and_leaves_crlf_and_blank_lines_alone() {
        let text = b"abc\r\n  def\r\n\r\n  ghi\r\n".to_vec();
        let source = SourceFile::new("a.rb", text.clone());
        let parsed = Parsed::parse(&source);
        let ctx = ctx_for(&source, &parsed);
        let span = Span::new(0, u32::try_from(text.len()).unwrap());

        let edits = shift_lines(&ctx, span, -2, &[]);

        let def_start = u32::try_from(text.windows(3).position(|w| w == b"  d").unwrap()).unwrap();
        let ghi_start = u32::try_from(text.windows(3).position(|w| w == b"  g").unwrap()).unwrap();
        // Line 1 has no leading whitespace to remove (its own line start is
        // the span start), and the blank line 3's whitespace-only guard
        // rejects the underflowing range into line 2's real content, so only
        // lines 2 and 4 produce a delete.
        assert_eq!(
            edits,
            vec![
                Edit::delete(Span::new(def_start, def_start + 2)),
                Edit::delete(Span::new(ghi_start, ghi_start + 2)),
            ]
        );
    }

    #[test]
    fn returns_no_edits_when_span_crosses_a_begin_end_block() {
        let text = b"foo\n=begin\ncomment\n=end\nbar\n".to_vec();
        let source = SourceFile::new("a.rb", text.clone());
        let parsed = Parsed::parse(&source);
        let ctx = ctx_for(&source, &parsed);
        let span = Span::new(0, u32::try_from(text.len()).unwrap());

        assert!(shift_lines(&ctx, span, 2, &[]).is_empty());
    }

    #[test]
    fn heredoc_bodies_covers_body_and_ignores_plain_strings() {
        let text = b"x = <<~SQL\n  SELECT *\n  FROM t\nSQL\ny = \"plain\"\n".to_vec();
        let source = SourceFile::new("a.rb", text.clone());
        let parsed = Parsed::parse(&source);
        let ctx = ctx_for(&source, &parsed);

        let bodies = heredoc_bodies(&ctx, &parsed.root());
        assert_eq!(bodies.len(), 1);
        let open_end =
            u32::try_from(text.windows(7).position(|w| w == b"<<~SQL\n").unwrap() + 6).unwrap();
        let close_start =
            u32::try_from(text.windows(4).rposition(|w| w == b"SQL\n").unwrap()).unwrap();
        assert_eq!(bodies[0], Span::new(open_end, close_start));
    }

    #[test]
    fn shift_lines_skips_lines_inside_a_heredoc_taboo() {
        let text = b"x = <<~SQL\n  SELECT *\n  FROM t\nSQL\n".to_vec();
        let source = SourceFile::new("a.rb", text.clone());
        let parsed = Parsed::parse(&source);
        let ctx = ctx_for(&source, &parsed);
        let root = parsed.root();
        let taboo = heredoc_bodies(&ctx, &root);
        let span = root.span();

        let edits = shift_lines(&ctx, span, 2, &taboo);

        // Only line 1 (the `x = <<~SQL` opener, anchored at the node's own
        // span start) is outside the heredoc-body taboo; every other line
        // -- including the closing `SQL` delimiter, whose start coincides
        // with the taboo's own end -- is skipped.
        assert_eq!(edits, vec![Edit::insert(span.start, b"  ".to_vec())]);
    }
}
