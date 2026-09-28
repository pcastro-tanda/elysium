//! `Layout/CommentIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/comment_indentation.rb` plus the `Alignment` mixin
//! it includes.
//!
//! This cop has no node traversal at all: every own-line comment is checked
//! against the raw text of the first non-blank line that follows it, using
//! only [`Context::comments`] and line text -- mirroring upstream's own
//! `on_new_investigation` (no `on_send`/`on_...` handlers).
//!
//! # Cascaded autocorrection
//!
//! A reported offense's fix ([`autocorrect_preceding_comments`]) also shifts
//! every immediately preceding comment line that shares the reported
//! comment's column, one line at a time, as long as each pair is on
//! consecutive lines at the same column (RuboCop's `should_correct?`). This
//! is why a whole block of same-indented comment lines directly above a
//! misindented statement produces exactly one offense (anchored at the
//! last comment, the one actually compared against real code) whose fix
//! silently repairs the rest: each of *those* comments' own `next_line` is
//! the comment below it, at the same (still wrong) indentation, so their own
//! `column_delta` computes to zero and they are never separately reported.

use linter::{
    Applicability, CommentInfo, CommentKind, ConfigDefault, ConfigOption, Context, Department,
    Edit, Fix, FixAvailability, OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_source::Span;

/// RuboCop's `MSG`.
fn message(column: u32, correct_comment_indentation: i64) -> String {
    format!(
        "Incorrect indentation detected (column {column} instead of \
         {correct_comment_indentation})."
    )
}

/// `Layout/AccessModifierIndentation`'s `EnforcedStyle` default.
const ACCESS_MODIFIER_STYLE_DEFAULT: &str = "indent";

/// Checks the indentation of comments.
#[derive(Debug, Clone)]
pub struct CommentIndentation {
    /// RuboCop's `cop_config['AllowForAlignment']`.
    allow_for_alignment: bool,
    /// RuboCop's `configured_indentation_width` (`Layout/IndentationWidth`'s
    /// `Width`, peered).
    indentation_width: i64,
    /// `Layout/AccessModifierIndentation`'s `EnforcedStyle == 'outdent'`,
    /// peered (RuboCop's `less_indented?`).
    access_modifier_outdent: bool,
}

impl CommentIndentation {
    /// RuboCop's `check`.
    fn check(&self, ctx: &mut Context<'_>, comments: &[CommentInfo], index: usize) {
        let comment = comments[index];
        if !own_line_comment(ctx, &comment) {
            return;
        }

        let next_line = line_after_comment(ctx, comment.line);
        let mut correct_comment_indentation = self.correct_indentation(ctx, next_line);
        let column = ctx.line_col(comment.span.start).column;

        let column_delta = correct_comment_indentation - i64::from(column);
        if column_delta == 0 {
            return;
        }

        let next_line_text = next_line.map(|span| ctx.text(span));
        if two_alternatives(next_line_text) {
            // Try the other.
            correct_comment_indentation += self.indentation_width;
            // `column_delta` stays unchanged so that autocorrect changes to
            // the preferred style of aligning the comment with the keyword.
            if i64::from(column) == correct_comment_indentation {
                return;
            }
        }

        if self.correctly_aligned_with_preceding_comment(ctx, comments, index, column) {
            return;
        }

        let delta = i32::try_from(column_delta).unwrap_or(0);
        let mut edits = autocorrect_preceding_comments(ctx, comments, index, delta);
        edits.extend(linter::shift_lines(ctx, comment.span, delta, &[]));

        let msg = message(column, correct_comment_indentation);
        if edits.is_empty() {
            ctx.report(&Self::META, comment.span, msg);
        } else {
            ctx.report_with_fix(
                &Self::META,
                comment.span,
                msg,
                Fix { applicability: Applicability::Safe, edits },
            );
        }
    }

    /// RuboCop's `correctly_aligned_with_preceding_comment?`.
    fn correctly_aligned_with_preceding_comment(
        &self,
        ctx: &Context<'_>,
        comments: &[CommentInfo],
        index: usize,
        column: u32,
    ) -> bool {
        if !self.allow_for_alignment {
            return false;
        }
        for other in comments[..index].iter().rev() {
            if !own_line_comment(ctx, other) {
                return ctx.line_col(other.span.start).column == column;
            }
        }
        false
    }

    /// RuboCop's `correct_indentation`.
    fn correct_indentation(&self, ctx: &Context<'_>, next_line: Option<Span>) -> i64 {
        let Some(span) = next_line else { return 0 };
        let text = ctx.text(span);
        let indentation_of_next_line =
            i64::try_from(text.iter().take_while(|&&b| is_ruby_space(b)).count()).unwrap_or(0);
        if less_indented(text, self.access_modifier_outdent) {
            indentation_of_next_line + self.indentation_width
        } else {
            indentation_of_next_line
        }
    }
}

/// RuboCop's `own_line_comment?`: the comment is a `#`-comment (not a
/// `=begin`/`=end` block) and only blanks precede it on its line.
fn own_line_comment(ctx: &Context<'_>, comment: &CommentInfo) -> bool {
    comment.kind == CommentKind::Inline && ctx.begins_its_line(comment.span)
}

/// RuboCop's `line_after_comment`: the byte span of the first line, after
/// the comment's own line, that is not empty/all-whitespace, or `None` when
/// no such line exists before the end of the file.
fn line_after_comment(ctx: &Context<'_>, comment_line: u32) -> Option<Span> {
    for line in (comment_line + 1)..=ctx.line_count() {
        let span = ctx.line_span(line);
        if !ctx.text(span).iter().all(|&b| is_ruby_space(b)) {
            return Some(span);
        }
    }
    None
}

/// RuboCop's `less_indented?`: the line starts (after leading whitespace)
/// with `end` (as a whole word) or a closing bracket, or -- only when
/// `Layout/AccessModifierIndentation` is configured to outdent -- with
/// `private`/`protected`/`public`.
fn less_indented(line: &[u8], access_modifier_outdent: bool) -> bool {
    let rest = skip_ruby_space(line);
    if starts_with_keyword(rest, b"end") || matches!(rest.first(), Some(b')' | b'}' | b']')) {
        return true;
    }
    access_modifier_outdent
        && (starts_with_keyword(rest, b"private")
            || starts_with_keyword(rest, b"protected")
            || starts_with_keyword(rest, b"public"))
}

/// RuboCop's `two_alternatives?`: the line starts (after leading
/// whitespace) with one of the keywords that can be indented either way
/// (aligned with their own line, or with the following statement).
fn two_alternatives(line: Option<&[u8]>) -> bool {
    let Some(line) = line else { return false };
    let rest = skip_ruby_space(line);
    [
        b"else".as_slice(),
        b"elsif".as_slice(),
        b"when".as_slice(),
        b"in".as_slice(),
        b"rescue".as_slice(),
        b"ensure".as_slice(),
    ]
    .iter()
    .any(|kw| starts_with_keyword(rest, kw))
}

/// RuboCop's `autocorrect_preceding_comments`: walks backward from the
/// reported comment through every immediately preceding comment that sits
/// on the line right above the previous one and shares its column,
/// shifting each by the same `delta` (the reported comment's own
/// `column_delta`) so the whole aligned block is repaired in one pass.
fn autocorrect_preceding_comments(
    ctx: &Context<'_>,
    comments: &[CommentInfo],
    index: usize,
    delta: i32,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    let mut below = comments[index];
    let mut i = index;
    while i > 0 {
        let above = comments[i - 1];
        if !should_correct(ctx, &above, &below) {
            break;
        }
        edits.extend(linter::shift_lines(ctx, above.span, delta, &[]));
        below = above;
        i -= 1;
    }
    edits
}

/// RuboCop's `should_correct?`.
fn should_correct(
    ctx: &Context<'_>,
    preceding_comment: &CommentInfo,
    reference_comment: &CommentInfo,
) -> bool {
    preceding_comment.line == reference_comment.line - 1
        && ctx.line_col(preceding_comment.span.start).column
            == ctx.line_col(reference_comment.span.start).column
}

/// Ruby's `\s` character class (ASCII-only, not full Unicode whitespace).
const fn is_ruby_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0x0C | 0x0B)
}

/// The part of `line` after its leading run of [`is_ruby_space`] bytes.
fn skip_ruby_space(line: &[u8]) -> &[u8] {
    let i = line.iter().take_while(|&&b| is_ruby_space(b)).count();
    &line[i..]
}

/// `rest` starts with the literal bytes of `word`, followed by a non-word
/// byte or the end of `rest` (Ruby's `\b` word boundary).
fn starts_with_keyword(rest: &[u8], word: &[u8]) -> bool {
    rest.len() >= word.len()
        && &rest[..word.len()] == word
        && rest.get(word.len()).is_none_or(|&b| !is_word_byte(b))
}

/// A byte that can be part of a Ruby identifier.
const fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

impl Rule for CommentIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/CommentIndentation",
        department: Department::Layout,
        summary: "Checks the indentation of comments.",
        explanation: "\
An own-line comment (one with only blanks before it on its line) is expected
to be indented like the code that follows it -- specifically, like the first
non-blank line after it, one extra level deeper when that line is a closing
`end`/`)`/`}`/`]` (the comment introduces the block being closed, not the
close itself).

```ruby
# bad
    # comment here
def method_name
end

# good
# comment here
def method_name
end
```

A comment directly before `else`/`elsif`/`when`/`in`/`rescue`/`ensure` may be
aligned with either that keyword or the statement above it:

```ruby
# good
if a
  b
# this is accepted
elsif aa
  # so is this
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[ConfigOption {
            name: "AllowForAlignment",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Allow comments to have extra indentation if that aligns them with a trailing \
                  comment on the nearest preceding non-own-line comment.",
        }],
        blind_spots: "\
`Layout/IndentationWidth`'s `Width` and `Layout/AccessModifierIndentation`'s
`EnforcedStyle` are read as peer options, matching upstream's own
cross-cop reads.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let allow_for_alignment = options.bool("AllowForAlignment");
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .unwrap_or(2);
        let access_modifier_outdent = options
            .peer("Layout/AccessModifierIndentation", "EnforcedStyle")
            .and_then(OptionValue::as_str)
            .unwrap_or(ACCESS_MODIFIER_STYLE_DEFAULT)
            == "outdent";
        Ok(Self { allow_for_alignment, indentation_width, access_modifier_outdent })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let comments: Vec<CommentInfo> = ctx.comments().to_vec();
        for index in 0..comments.len() {
            self.check(ctx, &comments, index);
        }
    }
}
