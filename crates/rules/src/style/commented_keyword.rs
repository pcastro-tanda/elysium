//! `Style/CommentedKeyword`, ported from RuboCop's
//! `lib/rubocop/cop/style/commented_keyword.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_source::Side;
use std::sync::LazyLock;

/// RuboCop's `KEYWORDS`: keywords that must not share a line with a
/// trailing comment.
const KEYWORDS: &[&str] = &["begin", "class", "def", "end", "module"];

/// RuboCop's `ALLOWED_COMMENTS` plus `DirectiveComment::DIRECTIVE_COMMENT_REGEXP`:
/// `:nodoc:`, `:yields:` and `# rubocop:disable/enable/todo/push/pop` are
/// exempt, whatever keyword they trail.
static ALLOWED_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#\s*:(nodoc|yields):").expect("valid"));

/// RuboCop's `DirectiveComment::DIRECTIVE_COMMENT_REGEXP`, simplified to a
/// boolean match: `# rubocop : (disable|enable|todo|push|pop)`, with `\s*`
/// wherever upstream's literal has a space.
static DIRECTIVE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#\s*rubocop\s*:\s*(disable|enable|todo|push|pop)\b").expect("valid")
});

/// RuboCop's `SUBCLASS_DEFINITION`.
static SUBCLASS_DEFINITION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A\s*class\s+(?:\w|::)+\s*<\s*(?:\w|::)+").expect("valid"));

/// RuboCop's `METHOD_OR_END_DEFINITIONS`.
static METHOD_OR_END_DEFINITIONS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A\s*(def\s|end)").expect("valid"));

/// RuboCop's rbs-inline `#[...]` annotation prefix check
/// (`comment.text.start_with?(/#\[.+\]/)`).
static RBS_GENERIC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A#\[.+\]").expect("valid"));

/// RuboCop's `STEEP_REGEXP`.
static STEEP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#\ssteep:ignore(\s|\z)").expect("valid"));

/// Do not place comments on the same line as certain keywords.
#[derive(Debug, Clone)]
pub struct CommentedKeyword;

impl Rule for CommentedKeyword {
    const META: RuleMeta = RuleMeta {
        name: "Style/CommentedKeyword",
        department: Department::Style,
        summary: "Do not place comments on the same line as certain keywords.",
        explanation: "Checks for comments put on the same line as some \
            keywords. These keywords are: `class`, `module`, `def`, \
            `begin`, `end`.\n\nNote that some comments (`:nodoc:`, \
            `:yields:`, `rubocop:disable` and `rubocop:todo`), RBS::Inline \
            annotation, and Steep annotation (`steep:ignore`) are allowed.\n\n\
            Autocorrection removes comments from `end` keyword and keeps \
            comments for `class`, `module`, `def` and `begin` above the \
            keyword.\n\n```ruby\n# bad\nif condition\n  statement\nend # end if\n\n\
            # bad\nclass X # comment\n  statement\nend\n\n# bad\ndef x; end # comment\n\n\
            # good\nif condition\n  statement\nend\n\n# good\nclass X # :nodoc:\n  y\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        for comment in ctx.comments().to_vec() {
            let line_text = ctx.line_text(comment.line);
            let Ok(line_str) = std::str::from_utf8(line_text) else { continue };
            let Some(keyword) = matched_keyword(line_str) else { continue };

            let comment_text = ctx.text(comment.span);
            let Ok(comment_str) = std::str::from_utf8(comment_text) else { continue };

            if rbs_inline_annotation(line_str, comment_str) {
                continue;
            }
            if STEEP_RE.is_match(comment_str) {
                continue;
            }
            if ALLOWED_COMMENT_RE.is_match(line_str) || DIRECTIVE_RE.is_match(line_str) {
                continue;
            }

            let message =
                format!("Do not place comments on the same line as the `{keyword}` keyword.");
            let removed = ctx.with_surrounding_space(comment.span, Side::Both, false, false);
            let mut edits = vec![Edit::delete(removed)];
            if keyword != "end" {
                let line_start = ctx.line_span(comment.line).start;
                let mut insertion = comment_str.as_bytes().to_vec();
                insertion.push(b'\n');
                edits.push(Edit::insert(line_start, insertion));
            }
            ctx.report_with_fix(
                &Self::META,
                comment.span,
                message,
                Fix { applicability: Applicability::Unsafe, edits },
            );
        }
    }
}

/// RuboCop's `KEYWORD_REGEXES.any? { |r| r.match?(line) }`, but also
/// returning which keyword matched -- `REGEXP`'s `(?<keyword>\S+)` capture
/// is always that same leading token, since a `KEYWORD_REGEXES` match
/// requires the keyword to be the line's first non-blank run.
fn matched_keyword(line: &str) -> Option<&'static str> {
    let bytes = line.as_bytes();
    let start = bytes.iter().position(|b| !b.is_ascii_whitespace())?;
    let rest = &bytes[start..];
    KEYWORDS.iter().copied().find(|kw| {
        rest.starts_with(kw.as_bytes()) && rest.get(kw.len()).is_some_and(u8::is_ascii_whitespace)
    })
}

/// RuboCop's `rbs_inline_annotation?`.
fn rbs_inline_annotation(line: &str, comment_text: &str) -> bool {
    if SUBCLASS_DEFINITION_RE.is_match(line) {
        RBS_GENERIC_RE.is_match(comment_text)
    } else if METHOD_OR_END_DEFINITIONS_RE.is_match(line) {
        comment_text.starts_with("#:")
    } else {
        false
    }
}
