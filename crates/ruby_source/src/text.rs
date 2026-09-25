//! Pure text predicates and [`SourceFile`] span helpers shared by every rule.
//!
//! These mirror small, frequently reimplemented corners of RuboCop and
//! `rubocop-ast`: the `\s` character class, `Util#comment_line?`,
//! `String#length`, `Node#single_line?`, `Range#last_line`,
//! `Util#begins_its_line?`, `RangeHelp#range_by_whole_lines`, and
//! `RangeHelp#range_with_surrounding_space`.

use crate::{SourceFile, Span};

/// Ruby's `\s` character class as used by RuboCop's own regexes (`/^\s*#/`,
/// `/^\s*$/`, ...): space, tab, newline, carriage return, vertical tab, form
/// feed. Notably broader than [`u8::is_ascii_whitespace`], which omits
/// vertical tab (`0x0B`).
#[must_use]
pub fn is_ruby_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

/// Character counterpart of [`is_ruby_whitespace`].
#[must_use]
pub fn is_ruby_whitespace_char(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{0B}' | '\u{0C}')
}

/// Ruby's `String#length`: the number of UTF-8 characters in `bytes`. Every
/// UTF-8 scalar has exactly one byte that is not a continuation byte
/// (`0b10xxxxxx`), so counting non-continuation bytes gives the exact
/// character count for valid UTF-8 and degrades to one "character" per
/// invalid byte, matching [`crate::LineIndex::line_col`]'s column counting.
#[must_use]
pub fn char_len(bytes: &[u8]) -> u32 {
    u32::try_from(bytes.iter().filter(|&&b| (b & 0xC0) != 0x80).count()).unwrap_or(u32::MAX)
}

/// RuboCop's `Util#comment_line?`: `/^\s*#/`, true when the first non-blank
/// byte on the line is `#`.
#[must_use]
pub fn is_comment_line(line: &[u8]) -> bool {
    match line.iter().position(|&b| !is_ruby_whitespace(b)) {
        Some(i) => line[i] == b'#',
        None => false,
    }
}

/// Which side(s) of a span [`SourceFile::with_surrounding_space`] expands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Expand only the span's start, leftward.
    Left,
    /// Expand only the span's end, rightward.
    Right,
    /// Expand both ends.
    Both,
}

/// Walks `pos` left over a run of plain spaces/tabs, then (if `newlines`) a
/// further run of bare `\n` bytes, then (if `whitespace`) a further run of
/// any [`is_ruby_whitespace`] byte. RuboCop's `RangeHelp#final_pos` stepping
/// left: three sequential passes, each independently gated.
fn walk_left(bytes: &[u8], mut pos: u32, newlines: bool, whitespace: bool) -> u32 {
    while pos > 0 && matches!(bytes[pos as usize - 1], b' ' | b'\t') {
        pos -= 1;
    }
    if newlines {
        while pos > 0 && bytes[pos as usize - 1] == b'\n' {
            pos -= 1;
        }
    }
    if whitespace {
        while pos > 0 && is_ruby_whitespace(bytes[pos as usize - 1]) {
            pos -= 1;
        }
    }
    pos
}

/// Mirror of [`walk_left`] stepping right.
fn walk_right(bytes: &[u8], mut pos: u32, newlines: bool, whitespace: bool) -> u32 {
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    while pos < len && matches!(bytes[pos as usize], b' ' | b'\t') {
        pos += 1;
    }
    if newlines {
        while pos < len && bytes[pos as usize] == b'\n' {
            pos += 1;
        }
    }
    if whitespace {
        while pos < len && is_ruby_whitespace(bytes[pos as usize]) {
            pos += 1;
        }
    }
    pos
}

impl SourceFile {
    /// `rubocop-ast`'s `Node#single_line?` (negated, `multiline?`): true when
    /// `span`'s start and last byte sit on the same source line.
    #[must_use]
    pub fn is_single_line(&self, span: Span) -> bool {
        let end = span.end.saturating_sub(1).max(span.start);
        self.line_col(span.start).line == self.line_col(end).line
    }

    /// RuboCop's `Range#last_line`: the 1-based line containing `span`'s
    /// last byte (its start line, for an empty span).
    #[must_use]
    pub fn last_line(&self, span: Span) -> u32 {
        self.line_col(span.end.saturating_sub(1).max(span.start)).line
    }

    /// True when `a` and `b` start on the same source line.
    #[must_use]
    pub fn same_line(&self, a: Span, b: Span) -> bool {
        self.line_col(a.start).line == self.line_col(b.start).line
    }

    /// RuboCop's `Util#begins_its_line?`: true when only blank characters
    /// precede `span`'s start on its own line.
    #[must_use]
    pub fn begins_its_line(&self, span: Span) -> bool {
        let line_col = self.line_col(span.start);
        let line = self.line_text(line_col.line);
        let Ok(text) = std::str::from_utf8(line) else { return line_col.column == 0 };
        match text.chars().position(|ch| !is_ruby_whitespace_char(ch)) {
            Some(index) => u32::try_from(index).unwrap_or(u32::MAX) == line_col.column,
            None => false,
        }
    }

    /// `rubocop-ast`'s `RangeHelp#range_by_whole_lines(range,
    /// include_final_newline: true)`: expands `span` to cover every whole
    /// line it touches, plus the line terminator after the last one when the
    /// file has more bytes after it.
    #[must_use]
    pub fn whole_lines(&self, span: Span) -> Span {
        let start_line = self.line_col(span.start).line;
        let last_included = span.end.saturating_sub(1).max(span.start);
        let end_line = self.line_col(last_included).line;
        let start = self.lines().line_start(start_line);
        let end_line_start = self.lines().line_start(end_line);
        let line_len = u32::try_from(self.line_text(end_line).len()).unwrap_or(u32::MAX);
        let line_end = end_line_start + line_len;
        let source_len = u32::try_from(self.bytes().len()).unwrap_or(u32::MAX);
        let end = if line_end < source_len { line_end + 1 } else { line_end };
        Span::new(start, end)
    }

    /// `rubocop-ast`'s `RangeHelp#range_with_surrounding_space`: expands
    /// `span` on the requested `side`(s). Mirrors `RangeHelp#final_pos`'s
    /// three sequential passes: a run of plain spaces/tabs; then, when
    /// `newlines` is set, a further run of bare `\n` bytes; then, when
    /// `whitespace` is set, a further run of any [`is_ruby_whitespace`]
    /// byte (space/tab/`\n`/`\r`/vertical tab/form feed) -- unlike the
    /// first two passes this one is a single generic walk, so with
    /// `whitespace: true` it can cross a bare newline and keep consuming
    /// further blank lines in the same call (matching
    /// `sole_nested_conditional::full_space_span` and
    /// `redundant_cop_disable_directive::swallow_right`/`swallow_left`'s
    /// `newlines: true` case). `SurroundingSpace#reposition` (used by
    /// `space_inside_array_literal_brackets`) and `ExtraSpacing`'s
    /// line-bounded, space-only `align_column` walk (`prev_visible_end`)
    /// are different RuboCop methods entirely and stay local to their
    /// rules.
    #[must_use]
    pub fn with_surrounding_space(
        &self,
        span: Span,
        side: Side,
        newlines: bool,
        whitespace: bool,
    ) -> Span {
        let bytes = self.bytes();
        let start = if matches!(side, Side::Left | Side::Both) {
            walk_left(bytes, span.start, newlines, whitespace)
        } else {
            span.start
        };
        let end = if matches!(side, Side::Right | Side::Both) {
            walk_right(bytes, span.end, newlines, whitespace)
        } else {
            span.end
        };
        Span::new(start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruby_whitespace_includes_vertical_tab_and_form_feed() {
        assert!(is_ruby_whitespace(0x0B));
        assert!(is_ruby_whitespace(0x0C));
        assert!(!is_ruby_whitespace(b'a'));
        assert!(is_ruby_whitespace_char('\u{0B}'));
        assert!(!is_ruby_whitespace_char('a'));
    }

    #[test]
    fn char_len_counts_unicode_scalars_not_bytes() {
        assert_eq!(char_len("héllo".as_bytes()), 5);
        assert_eq!(char_len(b""), 0);
    }

    #[test]
    fn char_len_is_lossy_on_invalid_utf8() {
        // Two non-continuation bytes -> two "characters", matching
        // LineIndex's own column counting for invalid input.
        assert_eq!(char_len(b"a\xff\xfeb"), 4);
    }

    #[test]
    fn is_comment_line_matches_leading_hash_after_blanks() {
        assert!(is_comment_line(b"  # comment"));
        assert!(is_comment_line(b"#!/usr/bin/env ruby"));
        assert!(!is_comment_line(b"x = 1 # not a comment line"));
        assert!(!is_comment_line(b"   "));
    }

    fn sf(src: &str) -> SourceFile {
        SourceFile::new("test.rb", src.as_bytes())
    }

    #[test]
    fn is_single_line_true_for_span_within_one_line_false_across_newline() {
        let source = sf("ab\ncd\n");
        assert!(source.is_single_line(Span::new(0, 2)));
        assert!(!source.is_single_line(Span::new(0, 4)));
    }

    #[test]
    fn is_single_line_true_for_empty_span_at_eof() {
        let source = sf("abc");
        let end = u32::try_from(source.bytes().len()).unwrap();
        assert!(source.is_single_line(Span::empty(end)));
    }

    #[test]
    fn last_line_of_span_ending_at_a_newline() {
        let source = sf("one\ntwo\nthree");
        // Span covering "one\n" (bytes 0..4): last byte is the newline itself, still line 1.
        assert_eq!(source.last_line(Span::new(0, 4)), 1);
        assert_eq!(source.last_line(Span::new(4, 8)), 2);
    }

    #[test]
    fn last_line_of_empty_span_is_its_own_line() {
        let source = sf("one\ntwo");
        assert_eq!(source.last_line(Span::empty(4)), 2);
    }

    #[test]
    fn same_line_compares_start_lines_only() {
        let source = sf("aa bb\ncc");
        assert!(source.same_line(Span::new(0, 2), Span::new(3, 5)));
        assert!(!source.same_line(Span::new(0, 2), Span::new(6, 8)));
    }

    #[test]
    fn begins_its_line_true_only_when_only_blanks_precede() {
        let source = sf("  foo\n bar baz");
        assert!(source.begins_its_line(Span::new(2, 5)));
        assert!(!source.begins_its_line(Span::new(9, 12)));
    }

    #[test]
    fn begins_its_line_counts_unicode_columns_not_bytes() {
        let source = sf("  héllo");
        // "héllo" starts at char column 2, byte offset 3 (é is 2 bytes).
        let start = source.line_text(1).len() - "héllo".len();
        let start = u32::try_from(start).unwrap();
        assert!(source.begins_its_line(Span::new(start, start + 1)));
    }

    #[test]
    fn whole_lines_expands_to_line_boundaries_with_trailing_newline() {
        let source = sf("one\ntwo\nthree\n");
        // Span covering just "wo" inside "two".
        let expanded = source.whole_lines(Span::new(5, 7));
        assert_eq!(expanded, Span::new(4, 8));
    }

    #[test]
    fn whole_lines_omits_final_newline_when_span_reaches_end_of_file() {
        let source = sf("one\ntwo");
        let expanded = source.whole_lines(Span::new(4, 7));
        assert_eq!(expanded, Span::new(4, 7));
    }

    #[test]
    fn with_surrounding_space_stops_at_first_newline_run_boundary() {
        // "a  \n  b": expanding the middle empty span rightward with
        // newlines crosses the blank run and the following bare newline,
        // but not the indentation spaces on the next line (RangeHelp's
        // `final_pos` does two *sequential* passes, not one mixed walk).
        let source = sf("a  \n  b");
        let expanded = source.with_surrounding_space(Span::empty(1), Side::Right, true, false);
        assert_eq!(expanded, Span::new(1, 4));
    }

    #[test]
    fn with_surrounding_space_horizontal_only_does_not_cross_newline() {
        let source = sf("a  \nb");
        let expanded = source.with_surrounding_space(Span::empty(1), Side::Right, false, false);
        assert_eq!(expanded, Span::new(1, 3));
    }

    #[test]
    fn with_surrounding_space_both_sides() {
        let source = sf("a  x  b");
        let expanded = source.with_surrounding_space(Span::new(3, 4), Side::Both, false, false);
        assert_eq!(expanded, Span::new(1, 6));
    }

    #[test]
    fn with_surrounding_space_whitespace_true_crosses_a_bare_newline() {
        // Unlike the two-phase (newlines: true) walk, `whitespace: true`
        // keeps consuming blanks after the newline in the same pass.
        let source = sf("a  \n  b");
        let expanded = source.with_surrounding_space(Span::empty(1), Side::Right, false, true);
        assert_eq!(expanded, Span::new(1, 6));
    }

    #[test]
    fn with_surrounding_space_whitespace_true_consumes_vertical_tab_and_form_feed() {
        let source = sf("a\u{0B}\u{0C}b");
        let expanded = source.with_surrounding_space(Span::empty(1), Side::Right, false, true);
        assert_eq!(expanded, Span::new(1, 3));
    }
}
