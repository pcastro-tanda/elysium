//! `Style/CommentAnnotation`, ported from RuboCop's
//! `lib/rubocop/cop/style/comment_annotation.rb`, with its
//! `RuboCop::Cop::AnnotationComment` helper (`lib/rubocop/cop/mixin/
//! annotation_comment.rb`) folded in privately as [`AnnotationComment`].

use linter::{
    CommentInfo, ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError,
    Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_source::Span;

const MSG_COLON_STYLE: &str = "Annotation keywords like `%s` should be all upper case, followed \
by a colon, and a space, then a note describing the problem.";
const MSG_SPACE_STYLE: &str = "Annotation keywords like `%s` should be all upper case, followed \
by a space, then a note describing the problem.";
const MISSING_NOTE: &str = "Annotation comment, with keyword `%s`, is missing a note.";

/// Ruby's `String#capitalize`: uppercase the first character, downcase the
/// rest.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// `RuboCop::Cop::AnnotationComment`: parses one comment's text against the
/// configured keywords, splitting it into margin/keyword/colon/space/note
/// groups exactly like upstream's regex.
struct AnnotationComment<'a> {
    keyword: Option<&'a str>,
    colon: Option<&'a str>,
    space: Option<&'a str>,
    note: Option<&'a str>,
    /// Byte offset (within `text`) where `keyword` starts, i.e. after the
    /// margin (`# ` or `#`).
    keyword_start: usize,
}

impl<'a> AnnotationComment<'a> {
    /// `split_comment`: matches `text` against the keyword regex, or fails
    /// to parse if it doesn't match at all.
    fn parse(text: &'a str, keyword_regex: &Regex) -> Option<Self> {
        let caps = keyword_regex.captures(text)?;
        let margin = caps.get(1)?;
        Some(Self {
            keyword: caps.get(2).map(|m| m.as_str()),
            colon: caps.get(3).map(|m| m.as_str()),
            space: caps.get(4).map(|m| m.as_str()),
            note: caps.get(5).map(|m| m.as_str()),
            keyword_start: margin.end(),
        })
    }

    /// `keyword_appearance?`: a keyword followed by a colon or space.
    fn keyword_appearance(&self) -> bool {
        self.keyword.is_some() && (self.colon.is_some() || self.space.is_some())
    }

    /// `just_keyword_of_sentence?`: a plain-capitalized word (e.g.
    /// `Optimize`) starting an ordinary sentence, not an annotation.
    fn just_keyword_of_sentence(&self) -> bool {
        let Some(keyword) = self.keyword else { return false };
        keyword == capitalize(keyword)
            && self.colon.is_none()
            && self.space.is_some()
            && self.note.is_some()
    }

    /// `annotation?`.
    fn is_annotation(&self) -> bool {
        self.keyword_appearance() && !self.just_keyword_of_sentence()
    }

    /// `correct?(colon:)`.
    fn is_correct(&self, requires_colon: bool) -> bool {
        let Some(keyword) = self.keyword else { return false };
        if self.space.is_none() || self.note.is_none() {
            return false;
        }
        if keyword != keyword.to_uppercase() {
            return false;
        }
        self.colon.is_some() == requires_colon
    }

    /// `bounds`: byte range (relative to the comment's own text) covering
    /// the keyword plus any colon and following space, but not the note.
    fn bounds(&self) -> (usize, usize) {
        let start = self.keyword_start;
        let length = self.keyword.map_or(0, str::len)
            + self.colon.map_or(0, str::len)
            + self.space.map_or(0, str::len);
        (start, start + length)
    }
}

/// Checks formatting of special comments (TODO, FIXME, OPTIMIZE, HACK, REVIEW, NOTE).
#[derive(Debug, Clone)]
pub struct CommentAnnotation {
    keyword_regex: Regex,
    require_colon: bool,
}

impl CommentAnnotation {
    /// `first_comment_line?`: true when `comments[index]` starts a fresh
    /// block of comments (either the very first comment, or preceded by a
    /// gap of more than one line).
    fn is_first_comment_line(comments: &[CommentInfo], index: usize) -> bool {
        index == 0 || comments[index - 1].line < comments[index].line - 1
    }
}

impl Rule for CommentAnnotation {
    const META: RuleMeta = RuleMeta {
        name: "Style/CommentAnnotation",
        department: Department::Style,
        summary:
            "Checks formatting of special comments (TODO, FIXME, OPTIMIZE, HACK, REVIEW, NOTE).",
        explanation: "\
Checks that comment annotation keywords are written according to
guidelines.

Annotation keywords can be specified by overriding the cop's `Keywords`
configuration. Keywords are allowed to be single words or phrases.

NOTE: With a multiline comment block (where each line is only a comment),
only the first line will be able to register an offense, even if an
annotation keyword starts another line. This is done to prevent incorrect
registering of keywords (e.g. `review`) inside a paragraph as an
annotation.

```ruby
# RequireColon: true (default)

# bad
# TODO make better

# good
# TODO: make better

# bad
# TODO:make better

# good
# TODO: make better

# bad
# fixme: does not work

# good
# FIXME: does not work

# bad
# Optimize does not work

# good
# OPTIMIZE: does not work

# RequireColon: false

# bad
# TODO: make better

# good
# TODO make better

# bad
# fixme does not work

# good
# FIXME does not work

# bad
# Optimize does not work

# good
# OPTIMIZE does not work
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[
            ConfigOption {
                name: "Keywords",
                default: ConfigDefault::StrList(&[
                    "TODO", "FIXME", "OPTIMIZE", "HACK", "REVIEW", "NOTE",
                ]),
                allowed: &[],
                doc: "Annotation keywords to check for formatting.",
            },
            ConfigOption {
                name: "RequireColon",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether annotation keywords must be followed by a colon.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let keywords = options.str_list("Keywords");
        let mut sorted: Vec<&str> = keywords.iter().map(String::as_str).collect();
        sorted.sort_by_key(|k| std::cmp::Reverse(k.len()));
        let union = sorted.iter().map(|k| regex::escape(k)).collect::<Vec<_>>().join("|");
        let pattern = format!(r"(?i)^(# ?)(\b(?:{union})\b)(\s*:)?(\s+)?(\S+)?");
        let keyword_regex = Regex::new(&pattern).expect("keyword regex is valid");
        Ok(Self { keyword_regex, require_colon: options.bool("RequireColon") })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let comments = ctx.comments().to_vec();
        for (index, comment) in comments.iter().enumerate() {
            let is_first = Self::is_first_comment_line(&comments, index);
            let is_inline = !ctx.begins_its_line(comment.span);
            if !is_first && !is_inline {
                continue;
            }

            let text = std::str::from_utf8(ctx.text(comment.span)).unwrap_or("");
            let Some(annotation) = AnnotationComment::parse(text, &self.keyword_regex) else {
                continue;
            };
            if !annotation.is_annotation() || annotation.is_correct(self.require_colon) {
                continue;
            }

            let keyword = annotation.keyword.unwrap_or_default();
            let message = if annotation.note.is_some() {
                let template = if self.require_colon { MSG_COLON_STYLE } else { MSG_SPACE_STYLE };
                template.replacen("%s", keyword, 1)
            } else {
                MISSING_NOTE.replacen("%s", keyword, 1)
            };

            let (start, end) = annotation.bounds();
            let base = comment.span.start;
            let span = Span::new(
                base + u32::try_from(start).expect("offset exceeds u32"),
                base + u32::try_from(end).expect("offset exceeds u32"),
            );
            ctx.report(&Self::META, span, message);
        }
    }
}
