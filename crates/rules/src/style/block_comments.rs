//! `Style/BlockComments`, ported from RuboCop's
//! `lib/rubocop/cop/style/block_comments.rb`.
//!
//! # Byte-range arithmetic vs upstream
//!
//! Upstream locates the `=begin`/`=end` markers with `Range#resize` and a
//! `comment.text.chomp == comment.text` check tied to the `parser` gem's
//! buffer quirks (a doc comment at EOF with no trailing newline gets an
//! `expression` range that runs one byte past the buffer). This engine's
//! [`CommentInfo::span`] (built from Prism) has no such quirk: it always
//! covers exactly `"=begin\n" + contents + "=end"`, plus a trailing `"\n"`
//! only when the source actually has one there. So instead of replaying
//! upstream's offset math, this port slices the comment's own text
//! directly off its known `"=begin\n"` prefix and `"=end"`/`"=end\n"`
//! suffix.

use linter::{
    Applicability, CommentKind, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Do not use block comments.";

/// RuboCop's `parts`/`eq_begin_part`: splits a `=begin\n...=end` comment's
/// text into the opening marker line, the contents between the markers,
/// and the closing marker (with its own trailing newline, if the source
/// has one). Byte offsets are relative to `full`.
fn split_marker(full: &[u8]) -> (usize, usize) {
    const BEGIN_LEN: usize = "=begin\n".len();
    let end_len = if full.ends_with(b"=end\n") { "=end\n".len() } else { "=end".len() };
    (BEGIN_LEN, full.len() - end_len)
}

/// RuboCop's autocorrect body: `contents.source.gsub(/\A/, '# ')
/// .gsub("\n\n", "\n#\n").gsub(/\n(?=[^#])/, "\n# ")`.
fn commentize(contents: &str) -> String {
    let prefixed = format!("# {contents}");
    let blank_lines_marked = prefixed.replace("\n\n", "\n#\n");
    let chars: Vec<char> = blank_lines_marked.chars().collect();
    let mut out = String::with_capacity(blank_lines_marked.len());
    for (i, &c) in chars.iter().enumerate() {
        out.push(c);
        if c == '\n' && chars.get(i + 1).is_some_and(|&next| next != '#') {
            out.push_str("# ");
        }
    }
    out
}

/// Do not use block comments.
#[derive(Debug, Clone)]
pub struct BlockComments;

impl Rule for BlockComments {
    const META: RuleMeta = RuleMeta {
        name: "Style/BlockComments",
        department: Department::Style,
        summary: "Do not use block comments.",
        explanation: "\
Looks for uses of block comments (`=begin`...`=end`).

```ruby
# bad
=begin
Multiple lines
of comments...
=end

# good
# Multiple lines
# of comments...
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let comments = ctx.comments().to_vec();
        for comment in comments {
            if comment.kind != CommentKind::EmbDoc {
                continue;
            }
            let full = ctx.text(comment.span);
            let (contents_start, contents_end) = split_marker(full);
            let contents = &full[contents_start..contents_end];

            let mut edits = vec![
                Edit::delete(Span::new(
                    comment.span.start,
                    comment.span.start + u32::try_from(contents_start).expect("small offset"),
                )),
                Edit::delete(Span::new(
                    comment.span.start + u32::try_from(contents_end).expect("small offset"),
                    comment.span.end,
                )),
            ];
            if !contents.is_empty() {
                let contents_str = String::from_utf8_lossy(contents);
                let replacement = commentize(&contents_str);
                edits.push(Edit::replace(
                    Span::new(
                        comment.span.start + u32::try_from(contents_start).expect("small offset"),
                        comment.span.start + u32::try_from(contents_end).expect("small offset"),
                    ),
                    replacement.into_bytes(),
                ));
            }

            ctx.report_with_fix(
                &Self::META,
                comment.span,
                MSG,
                Fix { applicability: Applicability::Safe, edits },
            );
        }
    }
}
