//! `Layout/EmptyComment`, ported from RuboCop's
//! `lib/rubocop/cop/layout/empty_comment.rb`.
//!
//! # Grouping instead of a joined regex
//!
//! Upstream's `concat_consecutive_comments` chunks adjacent comments
//! (`chunk_while` on "next line, same column") and joins each chunk's
//! `comment_text` (`text.strip + "\n"`) before matching it against
//! `/\A(#\n)+\z/` (`AllowBorderComment: true`) or `/\A(#+\n)+\z/`
//! (`AllowBorderComment: false`). Since every `comment_text` piece already
//! ends in the `\n` the pattern anchors on, and `#`/`#+` can never itself
//! match a `\n`, the only way the joined string decomposes into repeats of
//! the pattern's group is at the original per-comment boundaries -- so
//! whether the *whole chunk* matches reduces to "every comment in the
//! chunk, checked on its own, matches" ([`is_empty_comment_text`]). This
//! port checks chunks element-wise instead of building and matching the
//! joined string.
//!
//! When `AllowMarginComment` is `false`, upstream skips
//! `concat_consecutive_comments` entirely and checks each comment on its
//! own -- exactly a chunk of size one, so [`empty_comment_chunks`] just
//! stops growing a chunk past its first comment in that case.
//!
//! # `previous_token`/`same_line?` vs `begins_its_line`
//!
//! Upstream's autocorrect asks whether the token immediately before the
//! comment shares its line (`same_line?(node, previous_token)`) to decide
//! between removing the comment plus its surrounding horizontal space
//! (trailing on code) or the comment's whole line (standalone). This
//! engine has no token stream; [`Context::begins_its_line`] -- true when
//! only blanks precede the comment on its line -- is the same distinction
//! stated the other way around: a comment with a preceding token on the
//! same line never begins its own line, and one with nothing to its left
//! but whitespace never has a same-line previous token.

use linter::{
    Applicability, CommentInfo, ConfigDefault, ConfigOption, Context, Department, Edit, Fix,
    FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_source::{Side, Span};

/// RuboCop's `MSG`.
const MSG: &str = "Source code comment is empty.";

/// RuboCop's `empty_comment_only?`: does `text` (already `strip`ped, with a
/// trailing `\n` re-added by `comment_text`) consist of nothing but `#`
/// characters -- one bare `#` when `AllowBorderComment` is set, any run of
/// one or more `#` otherwise.
fn is_empty_comment_text(text: &[u8], allow_border_comment: bool) -> bool {
    if allow_border_comment {
        text == b"#"
    } else {
        !text.is_empty() && text.iter().all(|&b| b == b'#')
    }
}

/// Trims the same whitespace Ruby's `String#strip` does: ASCII blanks plus
/// `\0`, from both ends.
fn strip(bytes: &[u8]) -> &[u8] {
    let is_strip_char = |b: u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C | 0);
    let start = bytes.iter().position(|&b| !is_strip_char(b)).unwrap_or(bytes.len());
    let end = bytes.iter().rposition(|&b| !is_strip_char(b)).map_or(start, |i| i + 1);
    &bytes[start..end]
}

/// Checks empty comment.
#[derive(Debug, Clone)]
pub struct EmptyComment {
    allow_border_comment: bool,
    allow_margin_comment: bool,
}

impl EmptyComment {
    /// RuboCop's `concat_consecutive_comments` grouping (or, when
    /// `AllowMarginComment` is disabled, each comment alone): the next
    /// chunk boundary starting at `start`.
    fn chunk_end(&self, ctx: &Context<'_>, comments: &[CommentInfo], start: usize) -> usize {
        if !self.allow_margin_comment {
            return start + 1;
        }
        let mut end = start + 1;
        while end < comments.len() {
            let prev = &comments[end - 1];
            let cur = &comments[end];
            let prev_col = ctx.line_col(prev.span.start).column;
            let cur_col = ctx.line_col(cur.span.start).column;
            if cur.line == prev.line + 1 && cur_col == prev_col {
                end += 1;
            } else {
                break;
            }
        }
        end
    }

    /// RuboCop's `autocorrect`: remove the comment plus its surrounding
    /// horizontal space when it trails code on the same line, otherwise
    /// remove its whole line (including the line terminator).
    fn removal_span(ctx: &Context<'_>, span: Span) -> Span {
        if ctx.begins_its_line(span) {
            ctx.whole_lines(span)
        } else {
            ctx.with_surrounding_space(span, Side::Both, false, false)
        }
    }
}

impl Rule for EmptyComment {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyComment",
        department: Department::Layout,
        summary: "Checks empty comment.",
        explanation: "\
An empty `#` line carries no information and is usually leftover noise.

```ruby
# bad

#
class Foo
end

# good

#
# Description of `Foo` class.
#
class Foo
end
```

By default, a comment made entirely of `#` characters (a \"border\", e.g.
`#####`) and a bare `#` immediately adjacent to a real comment (a
\"margin\", used to frame it) are both left alone; `AllowBorderComment` and
`AllowMarginComment` turn either of those off.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[
            ConfigOption {
                name: "AllowBorderComment",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow comments that consist only of `#` characters (borders).",
            },
            ConfigOption {
                name: "AllowMarginComment",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow a bare `#` line adjacent to a non-empty comment (a margin).",
            },
        ],
        blind_spots: "\
Whether a comment trails code on its own line is approximated with
[`Context::begins_its_line`] rather than a real previous-token lookup;
this matches for every comment shape RuboCop itself considers (a comment
is always the last token on its line).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_border_comment: options.bool("AllowBorderComment"),
            allow_margin_comment: options.bool("AllowMarginComment"),
        })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let comments = ctx.comments().to_vec();
        let mut start = 0;
        while start < comments.len() {
            let end = self.chunk_end(ctx, &comments, start);
            let chunk = &comments[start..end];
            let all_empty = chunk.iter().all(|comment| {
                is_empty_comment_text(strip(ctx.text(comment.span)), self.allow_border_comment)
            });
            if all_empty {
                for comment in chunk {
                    let range = Self::removal_span(ctx, comment.span);
                    ctx.report_with_fix(
                        &Self::META,
                        comment.span,
                        MSG,
                        Fix {
                            applicability: Applicability::Safe,
                            edits: vec![Edit::delete(range)],
                        },
                    );
                }
            }
            start = end;
        }
    }
}
