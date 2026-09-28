//! `Lint/OrderedMagicComments`, ported from RuboCop's
//! `lib/rubocop/cop/lint/ordered_magic_comments.rb`, together with the
//! parts of its `RuboCop::MagicComment` dependency needed to distinguish
//! `encoding_specified?` from `frozen_string_literal_specified?` (RuboCop
//! has no per-file token stream available to us, so the comment-syntax
//! detection below is reimplemented directly against comment text; see
//! `Layout/EmptyLineAfterMagicComment`'s port for the `any?` subset of the
//! same logic).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::NodeExt;

/// RuboCop's `MSG`.
const MSG: &str = "The encoding magic comment should precede all other magic comments.";

/// Checks the proper ordering of magic comments and whether a magic comment is not placed before a shebang.
#[derive(Debug, Clone)]
pub struct OrderedMagicComments;

impl Rule for OrderedMagicComments {
    const META: RuleMeta = RuleMeta {
        name: "Lint/OrderedMagicComments",
        department: Department::Lint,
        summary: "Checks the proper ordering of magic comments and whether a magic comment is not placed before a shebang.",
        explanation: "\
Checks the proper ordering of magic comments and whether
a magic comment is not placed before a shebang.

@safety
  This cop's autocorrection is unsafe because file encoding may change.

```ruby
# bad

# frozen_string_literal: true
# encoding: ascii
p [''.frozen?, ''.encoding] #=> [true, #<Encoding:UTF-8>]

# good

# encoding: ascii
# frozen_string_literal: true
p [''.frozen?, ''.encoding] #=> [true, #<Encoding:US-ASCII>]

# good

#!/usr/bin/env ruby
# encoding: ascii
# frozen_string_literal: true
p [''.frozen?, ''.encoding] #=> [true, #<Encoding:US-ASCII>]
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "\
`RuboCop::MagicComment` is reimplemented against raw physical lines rather
than upstream's parser-gem-backed token stream, and only the
`encoding`/`frozen_string_literal` subset of its keywords is ported (the
other two, `Lint/OrderedMagicComments` never queries `rbs_inline`,
`shareable_constant_value`, or `typed`).",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        if ctx.source().bytes().is_empty() {
            return;
        }

        let Some((encoding_line, frozen_line)) = magic_comment_lines(ctx) else { return };
        if encoding_line < frozen_line {
            return;
        }

        let encoding_span = ctx.line_span(encoding_line);
        let frozen_span = ctx.line_span(frozen_line);
        let encoding_text = ctx.text(encoding_span).to_vec();
        let frozen_text = ctx.text(frozen_span).to_vec();

        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![
                Edit::replace(frozen_span, encoding_text),
                Edit::replace(encoding_span, frozen_text),
            ],
        };
        ctx.report_with_fix(&Self::META, encoding_span, MSG, fix);
    }
}

/// RuboCop's `magic_comment_lines`: the 1-based physical line of the first
/// leading line (among lines preceding the first non-comment token, or
/// every line if the file has none) for which `encoding_specified?` holds,
/// and likewise for `frozen_string_literal_specified?`, each keeping being
/// overwritten by later leading lines of the same kind until both are
/// found. Returns `None` unless both were found at least once.
fn magic_comment_lines(ctx: &Context<'_>) -> Option<(u32, u32)> {
    let boundary_line = first_non_comment_token_line(ctx);
    let last_leading_line = boundary_line.map_or(ctx.line_count(), |l| l.saturating_sub(1));

    let mut encoding_line = None;
    let mut frozen_line = None;
    for line in 1..=last_leading_line {
        let text = ctx.line_text(line);
        if encoding_specified(text) {
            encoding_line = Some(line);
        } else if frozen_string_literal_specified(text) {
            frozen_line = Some(line);
        }
        if encoding_line.is_some() && frozen_line.is_some() {
            break;
        }
    }
    Some((encoding_line?, frozen_line?))
}

/// 1-based line of the first token of the first top-level statement, i.e.
/// RuboCop's `processed_source.tokens.find { |token| !token.comment? }`
/// restricted to the common case of a top-level statement starting the
/// first non-comment token. `None` when the program has no statements at
/// all (whole file is leading comments/blank lines).
fn first_non_comment_token_line(ctx: &Context<'_>) -> Option<u32> {
    let program = ctx.parsed().root().as_program_node()?;
    let first = program.statements().body().iter().next()?;
    Some(ctx.line_col(first.span().start).line)
}

/// RuboCop's `MagicComment#encoding_specified?` (`specified?(encoding)`),
/// ported for the Emacs/Vim/plain comment dispatch.
fn encoding_specified(text: &[u8]) -> bool {
    if let Some(tokens) = emacs_tokens(text) {
        return tokens.iter().any(|&tok| editor_value(tok, kw_coding, b':'));
    }
    if let Some(rest) = vim_rest(text) {
        return vim_encoding_specified(rest);
    }
    simple_encoding(text)
}

/// RuboCop's `MagicComment#frozen_string_literal_specified?`
/// (`specified?(frozen_string_literal)`), ported for the Emacs/Vim/plain
/// comment dispatch. Vim comments can never specify it
/// (`VimComment#frozen_string_literal` is hard-overridden to `nil`).
fn frozen_string_literal_specified(text: &[u8]) -> bool {
    if let Some(tokens) = emacs_tokens(text) {
        return tokens.iter().any(|&tok| editor_value(tok, kw_frozen_string_literal, b':'));
    }
    if vim_rest(text).is_some() {
        return false;
    }
    simple_value(text, kw_frozen_string_literal).is_some()
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
/// [_-]literal`) matched at `b[i..]`.
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
/// `/io` flag on every `SimpleComment` extractor).
fn simple_value(text: &[u8], keyword: impl Fn(&[u8], usize, bool) -> Option<usize>) -> Option<()> {
    let mut i = skip_ws(text, 0);
    i = match_literal(text, i, b"#", true)?;
    i = skip_ws(text, i);
    i = keyword(text, i, true)?;
    i = match_literal(text, i, b":", true)?;
    i = skip_ws(text, i);
    let token_end = match_token(text, i)?;
    let after = skip_ws(text, token_end);
    (after == text.len()).then_some(())
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

/// RuboCop's `MagicComment.parse` dispatch to `EmacsComment`: the
/// `;`-separated, trimmed, non-empty tokens inside a `-*- ... -*-` marker,
/// or `None` when `text` isn't an Emacs-style comment at all.
fn emacs_tokens(text: &[u8]) -> Option<Vec<&[u8]>> {
    let start = find(text, b"-*-")?;
    let inner_start = start + 3;
    let end = rfind_from(text, b"-*-", inner_start)?;
    if end == inner_start {
        return None;
    }
    let inner = &text[inner_start..end];
    Some(inner.split(|&b| b == b';').map(trim).filter(|t| !t.is_empty()).collect())
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

/// Splits `b` on every occurrence of the literal separator `sep`, mirroring
/// Ruby's `String#split(sep)`.
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

/// RuboCop's `VimComment#encoding_specified?`: only true when there is more
/// than one `, `-separated token, and one of them is `fileencoding=...`.
fn vim_encoding_specified(rest: &[u8]) -> bool {
    let tokens: Vec<&[u8]> = split_seq(rest, b", ").map(trim).collect();
    if tokens.len() <= 1 {
        return false;
    }
    tokens.iter().any(|&tok| editor_value(tok, kw_fileencoding, b'='))
}
