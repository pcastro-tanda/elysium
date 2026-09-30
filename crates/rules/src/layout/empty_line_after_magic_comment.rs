//! `Layout/EmptyLineAfterMagicComment`, ported from RuboCop's
//! `lib/rubocop/cop/layout/empty_line_after_magic_comment.rb`, together with
//! the parts of its `RuboCop::MagicComment` dependency needed for
//! `MagicComment#any?` (RuboCop has no per-file token stream available to
//! us, so the comment-syntax detection below is reimplemented directly
//! against comment text; see `Style/FrozenStringLiteralComment`'s port for
//! the `frozen_string_literal`-only subset of the same logic).
//!
//! `NumberOfEmptyLines` (default `1`) is the minimum number of empty lines
//! required after the magic comment; `file_start` counts the run of blank
//! lines that already follows it (RuboCop's `empty_lines_after`) and only
//! offends -- inserting however many more are needed -- when that count
//! falls short, unless the whole rest of the file is blank (that trailing
//! run is `Layout/TrailingEmptyLines`'s responsibility, not this cop's).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::NodeExt;
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG_TEMPLATE: (&str, &str) = ("Expected at least ", " after magic comments; found ");

/// Checks for a newline after the final magic comment.
#[derive(Debug, Clone)]
pub struct EmptyLineAfterMagicComment {
    /// RuboCop's `number_of_empty_lines`.
    number_of_empty_lines: u32,
}

impl Rule for EmptyLineAfterMagicComment {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLineAfterMagicComment",
        department: Department::Layout,
        summary: "Checks for a newline after the final magic comment.",
        explanation: "\
Add an empty line after magic comments to separate them from the code.

`NumberOfEmptyLines` configures the minimum number of empty lines required.
Set it to `2` when using YARD, which otherwise treats the magic comments as
documentation for the first module or class in the file.

NOTE: `Layout/EmptyLines` has to be disabled for values greater than `1`, as
it removes the extra empty lines this cop adds, and autocorrecting with both
enabled loops between them.

```ruby
# good
# frozen_string_literal: true

# Some documentation for Person
class Person
  # Some code
end

# bad
# frozen_string_literal: true
# Some documentation for Person
class Person
  # Some code
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[ConfigOption {
            name: "NumberOfEmptyLines",
            default: ConfigDefault::Int(1),
            allowed: &[],
            doc: "The minimum number of empty lines required after magic comments.",
        }],
        blind_spots: "\
`RuboCop::MagicComment` is reimplemented against raw comment text rather
than upstream's parser-gem-backed wrapper classes. Every keyword
(`frozen_string_literal`, `encoding`/`coding`, `rbs_inline`, `warn_indent`,
`shareable_constant_value`, `typed`) and all three comment syntaxes (plain,
Emacs `-*- ... -*-`, Vim `# vim: ...`) are ported, including the
`rbs_inline` restriction to a literal `enabled`/`disabled` value and the
case-sensitivity difference between the plain-comment patterns (RuboCop's
`/io` flag) and the Emacs/Vim ones (no flag).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let raw = options.int("NumberOfEmptyLines");
        if raw <= 0 {
            return Err(OptionError {
                rule: Self::META.name,
                option: "NumberOfEmptyLines".to_string(),
                message: format!(
                    "The `Layout/EmptyLineAfterMagicComment` cop only accepts a positive \
                     integer for its `NumberOfEmptyLines` configuration parameter, but `{raw}` \
                     was given."
                ),
            });
        }
        let number_of_empty_lines = u32::try_from(raw).unwrap_or(1);
        Ok(Self { number_of_empty_lines })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let Some(magic_line) = last_magic_comment_line(ctx) else { return };
        let expected = self.number_of_empty_lines;
        let actual = empty_lines_after(ctx, magic_line);
        // Trailing empty lines are `Layout/TrailingEmptyLines`'s responsibility.
        if magic_line + actual + 1 > ctx.line_count() {
            return;
        }
        if actual >= expected {
            return;
        }

        let next_line = magic_line + 1;
        let start = ctx.line_span(next_line).start;
        let offending_range = Span::new(start, start + 1);
        let deficit = expected - actual;
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::insert(start, "\n".repeat(deficit as usize).into_bytes())],
        };
        let (prefix, suffix) = MSG_TEMPLATE;
        let unit = if expected == 1 { "line" } else { "lines" };
        let message = format!("{prefix}{expected} empty {unit}{suffix}{actual}.");
        ctx.report_with_fix(&Self::META, offending_range, message, fix);
    }
}

/// RuboCop's `empty_lines_after`: the number of consecutive blank lines
/// immediately following `magic_line` (the last magic comment's own line).
fn empty_lines_after(ctx: &Context<'_>, magic_line: u32) -> u32 {
    let mut count = 0;
    loop {
        let line = magic_line + 1 + count;
        if line > ctx.line_count() || !ctx.line_text(line).iter().all(u8::is_ascii_whitespace) {
            break;
        }
        count += 1;
    }
    count
}

/// RuboCop's `last_magic_comment`: among the comments preceding the first
/// top-level statement (or every comment, if the file has none), the
/// 1-based line of the last one for which `MagicComment.parse(text).any?`
/// holds.
fn last_magic_comment_line(ctx: &Context<'_>) -> Option<u32> {
    let program = ctx.parsed().root().as_program_node()?;
    let code_line = match program.statements().body().iter().next() {
        Some(first) => ctx.line_col(first.span().start).line,
        None => u32::MAX,
    };
    ctx.comments()
        .iter()
        .filter(|c| c.line < code_line)
        .rev()
        .find(|c| is_magic_comment(ctx.text(c.span)))
        .map(|c| c.line)
}

/// RuboCop's `MagicComment.parse(text).any?`: whether the comment `text`
/// (including its leading `#`) specifies any of the six recognized
/// settings, under whichever of the three comment syntaxes it matches.
fn is_magic_comment(text: &[u8]) -> bool {
    if let Some(any) = emacs_any(text) {
        return any;
    }
    if let Some(rest) = vim_rest(text) {
        return vim_any(rest);
    }
    simple_any(text)
}

/// Ruby's `\s` character class (ASCII-only), as used by every pattern below.
fn is_ruby_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0x0C | 0x0B)
}

/// The end of the run of [`is_ruby_space`] bytes starting at `b[i..]`.
fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && is_ruby_space(b[i]) {
        i += 1;
    }
    i
}

/// Matches the literal `lit` at `b[i..]`, case-insensitively when `ci`.
fn match_literal(b: &[u8], i: usize, lit: &[u8], ci: bool) -> Option<usize> {
    let end = i.checked_add(lit.len())?;
    if end > b.len() {
        return None;
    }
    let slice = &b[i..end];
    let matched = if ci { slice.eq_ignore_ascii_case(lit) } else { slice == lit };
    matched.then_some(end)
}

/// One of RuboCop's `[_-]`-joined keyword patterns (`frozen[_-]string
/// [_-]literal`, `shareable[_-]constant[_-]value`) matched at `b[i..]`.
fn match_segmented(b: &[u8], mut i: usize, segments: &[&[u8]], ci: bool) -> Option<usize> {
    for (idx, seg) in segments.iter().enumerate() {
        if idx > 0 {
            match b.get(i) {
                Some(b'_' | b'-') => i += 1,
                _ => return None,
            }
        }
        i = match_literal(b, i, seg, ci)?;
    }
    Some(i)
}

/// RuboCop's `TOKEN` (`[[:alnum:]\-_]+`): the run of ASCII alphanumerics,
/// `-`, and `_` starting at `b[i..]`.
fn match_token(b: &[u8], i: usize) -> Option<usize> {
    let mut end = i;
    while end < b.len() && (b[end].is_ascii_alphanumeric() || matches!(b[end], b'-' | b'_')) {
        end += 1;
    }
    (end > i).then_some(end)
}

/// RuboCop's `KEYWORDS[:frozen_string_literal]`.
fn kw_frozen_string_literal(b: &[u8], i: usize, ci: bool) -> Option<usize> {
    match_segmented(b, i, &[b"frozen", b"string", b"literal"], ci)
}

/// RuboCop's `KEYWORDS[:shareable_constant_value]`.
fn kw_shareable_constant_value(b: &[u8], i: usize, ci: bool) -> Option<usize> {
    match_segmented(b, i, &[b"shareable", b"constant", b"value"], ci)
}

/// RuboCop's `KEYWORDS[:warn_indent]`.
fn kw_warn_indent(b: &[u8], i: usize, ci: bool) -> Option<usize> {
    match_segmented(b, i, &[b"warn", b"indent"], ci)
}

/// RuboCop's `KEYWORDS[:rbs_inline]`.
fn kw_rbs_inline(b: &[u8], i: usize, ci: bool) -> Option<usize> {
    match_literal(b, i, b"rbs_inline", ci)
}

/// RuboCop's `KEYWORDS[:typed]`.
fn kw_typed(b: &[u8], i: usize, ci: bool) -> Option<usize> {
    match_literal(b, i, b"typed", ci)
}

/// RuboCop's `KEYWORDS[:encoding]` (`(?:en)?coding`).
fn kw_coding(b: &[u8], i: usize, ci: bool) -> Option<usize> {
    match_literal(b, i, b"encoding", ci).or_else(|| match_literal(b, i, b"coding", ci))
}

/// Vim's overridden `KEYWORDS[:encoding]` (`fileencoding`).
fn kw_fileencoding(b: &[u8], i: usize, ci: bool) -> Option<usize> {
    match_literal(b, i, b"fileencoding", ci)
}

/// RuboCop's `SimpleComment` keyword pattern anchored over the whole
/// comment: `\A\s*#\s*<keyword>:\s*TOKEN\s*\z`, case-insensitively (the
/// `/io` flag on every `SimpleComment` extractor). Returns the raw
/// (un-downcased) token text when the whole comment matches.
fn simple_value(
    text: &[u8],
    keyword: impl Fn(&[u8], usize, bool) -> Option<usize>,
) -> Option<&[u8]> {
    let mut i = skip_ws(text, 0);
    i = match_literal(text, i, b"#", true)?;
    i = skip_ws(text, i);
    i = keyword(text, i, true)?;
    i = match_literal(text, i, b":", true)?;
    i = skip_ws(text, i);
    let token_start = i;
    let token_end = match_token(text, i)?;
    let after = skip_ws(text, token_end);
    (after == text.len()).then(|| &text[token_start..token_end])
}

/// RuboCop's `SimpleComment#encoding`: `\A\s*#\s*(frozen_string_literal:
/// \s*(true|false))?\s*(?:en)?coding: TOKEN`, unanchored at the end (and
/// with a literal single space, not `\s*`, before `TOKEN`).
fn simple_encoding(text: &[u8]) -> bool {
    let mut i = skip_ws(text, 0);
    let Some(after_hash) = match_literal(text, i, b"#", true) else { return false };
    i = skip_ws(text, after_hash);
    if let Some(after_kw) = kw_frozen_string_literal(text, i, true) {
        if let Some(after_colon) = match_literal(text, after_kw, b":", true) {
            let after_ws = skip_ws(text, after_colon);
            if let Some(after_val) = match_literal(text, after_ws, b"true", true)
                .or_else(|| match_literal(text, after_ws, b"false", true))
            {
                i = after_val;
            }
        }
    }
    i = skip_ws(text, i);
    let Some(after_kw) = kw_coding(text, i, true) else { return false };
    let Some(after_colon) = match_literal(text, after_kw, b":", true) else { return false };
    let Some(after_space) = match_literal(text, after_colon, b" ", true) else { return false };
    match_token(text, after_space).is_some()
}

/// RuboCop's `SimpleComment#any?`.
fn simple_any(text: &[u8]) -> bool {
    if simple_value(text, kw_frozen_string_literal).is_some() {
        return true;
    }
    if simple_encoding(text) {
        return true;
    }
    if matches!(simple_value(text, kw_rbs_inline), Some(b"enabled" | b"disabled")) {
        return true;
    }
    if simple_value(text, kw_warn_indent).is_some() {
        return true;
    }
    if simple_value(text, kw_shareable_constant_value).is_some() {
        return true;
    }
    simple_value(text, kw_typed).is_some()
}

/// Byte offset of the first occurrence of `needle` in `haystack`, if any.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Byte offset of the last occurrence of `needle` in `haystack[from..]`
/// (relative to `haystack`), if any.
fn rfind_from(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from > haystack.len() || needle.is_empty() || haystack.len() - from < needle.len() {
        return None;
    }
    haystack[from..].windows(needle.len()).rposition(|w| w == needle).map(|i| from + i)
}

/// Ruby's `String#strip`, restricted to [`is_ruby_space`].
fn trim(b: &[u8]) -> &[u8] {
    let start = b.iter().position(|&c| !is_ruby_space(c)).unwrap_or(b.len());
    let end = b.iter().rposition(|&c| !is_ruby_space(c)).map_or(start, |p| p + 1);
    &b[start..end]
}

/// RuboCop's `EditorComment#match`: `\A<keyword>\s*<operator>\s*TOKEN\z`,
/// case-sensitively (no `/i` flag), applied to one already-trimmed token.
fn editor_value(
    tok: &[u8],
    keyword: impl Fn(&[u8], usize, bool) -> Option<usize>,
    operator: u8,
) -> bool {
    let Some(after_kw) = keyword(tok, 0, false) else { return false };
    let i = skip_ws(tok, after_kw);
    let Some(after_op) = match_literal(tok, i, &[operator], false) else { return false };
    let i = skip_ws(tok, after_op);
    match_token(tok, i).is_some_and(|end| end == tok.len())
}

/// RuboCop's `MagicComment.parse` dispatch to `EmacsComment` plus its
/// `any?` (`frozen_string_literal_specified? || encoding_specified? ||
/// warn_indent_specified? || shareable_constant_value_specified?`; Emacs
/// comments never specify `rbs_inline` or `typed`). Returns `None` when
/// `text` isn't an Emacs-style `-*- ... -*-` comment at all.
fn emacs_any(text: &[u8]) -> Option<bool> {
    let start = find(text, b"-*-")?;
    let inner_start = start + 3;
    let end = rfind_from(text, b"-*-", inner_start)?;
    if end == inner_start {
        return None;
    }
    let inner = &text[inner_start..end];
    Some(inner.split(|&b| b == b';').any(|tok| {
        let tok = trim(tok);
        !tok.is_empty()
            && (editor_value(tok, kw_frozen_string_literal, b':')
                || editor_value(tok, kw_coding, b':')
                || editor_value(tok, kw_warn_indent, b':')
                || editor_value(tok, kw_shareable_constant_value, b':'))
    }))
}

/// RuboCop's `VimComment::REGEXP` (`#\s*vim:\s*(?<token>.+)`): the text
/// following the first `#\s*vim:\s*` marker found anywhere in `text`, if
/// any.
fn vim_rest(text: &[u8]) -> Option<&[u8]> {
    for (idx, &b) in text.iter().enumerate() {
        if b != b'#' {
            continue;
        }
        let i = skip_ws(text, idx + 1);
        if let Some(after) = match_literal(text, i, b"vim:", false) {
            let after = skip_ws(text, after);
            return Some(&text[after..]);
        }
    }
    None
}

/// Splits `b` on every occurrence of the literal (possibly multi-byte)
/// separator `sep`, mirroring Ruby's `String#split(sep)`.
fn split_seq<'a>(b: &'a [u8], sep: &'static [u8]) -> impl Iterator<Item = &'a [u8]> {
    let mut rest = Some(b);
    std::iter::from_fn(move || {
        let cur = rest?;
        if let Some(pos) = find(cur, sep) {
            rest = Some(&cur[pos + sep.len()..]);
            Some(&cur[..pos])
        } else {
            rest = None;
            Some(cur)
        }
    })
}

/// RuboCop's `VimComment#any?`: only `encoding_specified?` can ever be
/// true (every other setting is hard-overridden to `nil`/unspecified), and
/// only when there is more than one `, `-separated token.
fn vim_any(rest: &[u8]) -> bool {
    let tokens: Vec<&[u8]> = split_seq(rest, b", ").map(trim).collect();
    if tokens.len() <= 1 {
        return false;
    }
    tokens.iter().any(|&tok| editor_value(tok, kw_fileencoding, b'='))
}
