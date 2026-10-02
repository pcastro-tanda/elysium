//! `Lint/MisplacedMagicComment`, ported from RuboCop's
//! `lib/rubocop/cop/lint/misplaced_magic_comment.rb`, together with the
//! parts of its `RuboCop::MagicComment` dependency this cop uses: comment
//! shape detection and the `encoding`/`frozen_string_literal` (plus, for
//! `valid?`'s `any?`, the other four keywords it checks only to decide
//! whether a *preceding* comment counts as "a magic comment") extraction.
//! RuboCop has no per-file token stream available to us, so the
//! comment-syntax detection below is reimplemented directly against comment
//! text, following the same approach as `lint/ordered_magic_comments.rs`
//! (its sibling cop, sharing the same `RuboCop::MagicComment` dependency).

use linter::{
    Applicability, CommentInfo, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::NodeExt as _;
use ruby_source::Side;

/// Upstream's `MSG_ENCODING`.
const MSG_ENCODING: &str = "The `encoding` magic comment is ignored unless placed on the first \
                             line (or below a shebang on the first line).";
/// Upstream's `MSG_AFTER_CODE` (`%<directive>s` always `frozen_string_literal`).
const MSG_AFTER_CODE: &str = "The `frozen_string_literal` magic comment is ignored after any code.";
/// Upstream's `MSG_ABOVE_SHEBANG`.
const MSG_ABOVE_SHEBANG: &str = "A magic comment above a shebang renders the shebang ineffective.";

/// Checks for magic comments placed where Ruby silently ignores them.
#[derive(Debug, Clone)]
pub struct MisplacedMagicComment;

impl Rule for MisplacedMagicComment {
    const META: RuleMeta = RuleMeta {
        name: "Lint/MisplacedMagicComment",
        department: Department::Lint,
        summary: "Checks for magic comments placed where Ruby silently ignores them.",
        explanation: "\
The `encoding` magic comment is only honored on the first line of a file,
or on the second line when the first line is a shebang. Anywhere else it
is silently ignored - even directly below another comment or a blank line.

Other magic comments (such as `frozen_string_literal`) are honored
anywhere before the first token of code, but are ignored after any code,
with a warning emitted only when running Ruby with `-w`.

A magic comment misplaced ahead of a shebang also renders the shebang
ineffective, since a shebang is only recognized on the first line.

NOTE: An `encoding` comment that is only preceded by other magic
comments is not flagged by this cop; that case is handled by
`Lint/OrderedMagicComments`. `shareable_constant_value` is never
flagged, as Ruby intentionally allows it mid-file with block scoping.

@safety
  This cop's autocorrection is unsafe because moving the magic comment
  to its effective position activates it, which changes runtime
  behavior (e.g. the source encoding or string mutability).

```ruby
# bad
# Documentation comment
# encoding: ascii-8bit
puts 'hello'

# good
# encoding: ascii-8bit
# Documentation comment
puts 'hello'

# bad
require 'foo'
# frozen_string_literal: true

# good
# frozen_string_literal: true
require 'foo'

# bad
# frozen_string_literal: true
#!/usr/bin/env ruby
puts 'hello'

# good
#!/usr/bin/env ruby
# frozen_string_literal: true
puts 'hello'
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "\
`RuboCop::MagicComment` is reimplemented against raw physical lines rather \
than upstream's parser-gem-backed token stream, following \
`lint/ordered_magic_comments.rs`'s port of the same dependency. \
`known_encoding?` consults a fixed list of Ruby's built-in encoding names \
and aliases rather than calling `Encoding.find`, so an encoding name only a \
loaded external `Encoding` subclass would recognize is treated as unknown \
prose instead of a real directive.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        if ctx.source().bytes().is_empty() {
            return;
        }
        check_magic_comment_above_shebang(ctx);
        let comments: Vec<CommentInfo> = ctx.comments().to_vec();
        for comment in comments {
            check_comment(ctx, comment);
        }
    }
}

fn check_comment(ctx: &mut Context<'_>, comment: CommentInfo) {
    let text = ctx.text(comment.span).to_vec();
    if !magic_comment_shaped(&text) {
        return;
    }
    if let Some(encoding) = encoding_value(&text) {
        if !known_encoding(&encoding) {
            return;
        }
        check_encoding_comment(ctx, comment, &text);
    } else if specified_colon(&text, kw_frozen_string_literal) {
        check_top_block_comment(ctx, comment, &text);
    }
}

/// Upstream's `magic_comment_shaped?`: the comment text starts with `#` and
/// has no second `#` anywhere, so a documentation comment that merely
/// quotes a magic comment never matches.
fn magic_comment_shaped(text: &[u8]) -> bool {
    text.first() == Some(&b'#') && !text[1..].contains(&b'#')
}

/// Upstream's `check_encoding_comment`.
fn check_encoding_comment(ctx: &mut Context<'_>, comment: CommentInfo, text: &[u8]) {
    let line = comment.line;
    let effective_line = effective_encoding_line(ctx);
    if line == effective_line && comment_starts_line(ctx, line) {
        return;
    }
    if preceded_only_by_magic_comments(ctx, line) {
        return;
    }
    let fix = move_comment(ctx, comment, text, effective_line);
    ctx.report_with_fix(&MisplacedMagicComment::META, comment.span, MSG_ENCODING, fix);
}

/// Upstream's `check_top_block_comment` (`directive` is always
/// `frozen_string_literal`, the only caller).
fn check_top_block_comment(ctx: &mut Context<'_>, comment: CommentInfo, text: &[u8]) {
    let Some(first_code) = first_code_token_start(ctx) else { return };
    if comment.span.start <= first_code {
        return;
    }
    let effective_line = effective_encoding_line(ctx);
    let fix = move_comment(ctx, comment, text, effective_line);
    ctx.report_with_fix(&MisplacedMagicComment::META, comment.span, MSG_AFTER_CODE, fix);
}

/// Upstream's `check_magic_comment_above_shebang`.
fn check_magic_comment_above_shebang(ctx: &mut Context<'_>) {
    let shebang = ctx.comments().iter().copied().find(|c| {
        c.line > 1 && ctx.text(c.span).starts_with(b"#!") && ctx.line_col(c.span.start).column == 0
    });
    let Some(shebang) = shebang else { return };
    if !preceded_only_by_magic_comments(ctx, shebang.line) {
        return;
    }
    ctx.report(&MisplacedMagicComment::META, shebang.span, MSG_ABOVE_SHEBANG);
}

/// Upstream's `shebang?`.
fn has_shebang(ctx: &Context<'_>) -> bool {
    ctx.line_text(1).starts_with(b"#!")
}

/// Upstream's `effective_encoding_line`.
fn effective_encoding_line(ctx: &Context<'_>) -> u32 {
    if has_shebang(ctx) {
        2
    } else {
        1
    }
}

/// Upstream's `comment_starts_line?`.
fn comment_starts_line(ctx: &Context<'_>, line: u32) -> bool {
    let text = ctx.line_text(line);
    let trimmed_start = text.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(text.len());
    text[trimmed_start..].starts_with(b"#")
}

/// The byte offset of the first top-level statement's first token --
/// upstream's `processed_source.tokens.find { |token| !token.comment? }`,
/// restricted to the common case where that token begins a top-level
/// statement. `None` when the program has no statements (an all-comments
/// file).
fn first_code_token_start(ctx: &Context<'_>) -> Option<u32> {
    let program = ctx.parsed().root().as_program_node()?;
    let first = program.statements().body().iter().next()?;
    Some(first.span().start)
}

/// Upstream's `preceded_only_by_magic_comments?`.
fn preceded_only_by_magic_comments(ctx: &Context<'_>, line: u32) -> bool {
    (1..line).all(|line_number| {
        let text = ctx.line_text(line_number);
        (line_number == 1 && text.starts_with(b"#!")) || is_valid_magic_comment(text)
    })
}

/// Upstream's `MagicComment#valid?` (`start_with?('#') && any?`).
fn is_valid_magic_comment(text: &[u8]) -> bool {
    text.first() == Some(&b'#') && any_magic_comment(text)
}

/// Upstream's `MagicComment#any?`.
fn any_magic_comment(text: &[u8]) -> bool {
    encoding_value(text).is_some()
        || specified_colon(text, kw_frozen_string_literal)
        || specified_colon(text, kw_rbs_inline)
        || specified_colon(text, kw_warn_indent)
        || specified_colon(text, kw_shareable_constant_value)
        || specified_colon(text, kw_typed)
}

/// Upstream's `move_comment`.
#[allow(clippy::cast_possible_truncation)]
fn move_comment(ctx: &Context<'_>, comment: CommentInfo, text: &[u8], target_line: u32) -> Fix {
    let removal_span = if comment_starts_line(ctx, comment.line) {
        ctx.whole_lines(comment.span)
    } else {
        ctx.with_surrounding_space(comment.span, Side::Left, true, false)
    };
    let target_span = ctx.line_span(target_line);
    let mut edits = vec![Edit::delete(removal_span)];
    if comment.line < target_line {
        let mut insertion = Vec::with_capacity(text.len() + 1);
        insertion.push(b'\n');
        insertion.extend_from_slice(text);
        edits.push(Edit::insert(target_span.end, insertion));
    } else {
        let mut insertion = text.to_vec();
        insertion.push(b'\n');
        edits.push(Edit::insert(target_span.start, insertion));
    }
    Fix { applicability: Applicability::Unsafe, edits }
}

// ---------------------------------------------------------------------
// `RuboCop::MagicComment` port: shared with `lint/ordered_magic_comments.rs`
// (copied privately, see this crate's porting rules).
// ---------------------------------------------------------------------

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

/// RuboCop's `KEYWORDS[:rbs_inline]`.
fn kw_rbs_inline(b: &[u8], i: usize, ci: bool) -> Option<usize> {
    match_literal(b, i, b"rbs_inline", ci)
}

/// RuboCop's `KEYWORDS[:warn_indent]`.
fn kw_warn_indent(b: &[u8], i: usize, ci: bool) -> Option<usize> {
    match_segmented(b, i, &[b"warn", b"indent"], ci)
}

/// RuboCop's `KEYWORDS[:shareable_constant_value]`.
fn kw_shareable_constant_value(b: &[u8], i: usize, ci: bool) -> Option<usize> {
    match_segmented(b, i, &[b"shareable", b"constant", b"value"], ci)
}

/// RuboCop's `KEYWORDS[:typed]`.
fn kw_typed(b: &[u8], i: usize, ci: bool) -> Option<usize> {
    match_literal(b, i, b"typed", ci)
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
fn simple_encoding_value(text: &[u8]) -> Option<Vec<u8>> {
    let mut i = skip_ws(text, 0);
    let after_hash = match_literal(text, i, b"#", true)?;
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
    let after_kw = kw_coding(text, i, true)?;
    let after_colon = match_literal(text, after_kw, b":", true)?;
    let after_space = match_literal(text, after_colon, b" ", true)?;
    let end = match_token(text, after_space)?;
    Some(text[after_space..end].to_vec())
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

/// RuboCop's `EditorComment#match`, as a boolean `specified?` check:
/// `\A<keyword>\s*<operator>\s*TOKEN\z`, case-sensitively (no `/i` flag),
/// applied to one already-trimmed token.
fn editor_value(
    tok: &[u8],
    keyword: impl Fn(&[u8], usize, bool) -> Option<usize>,
    operator: u8,
) -> bool {
    editor_match_value(tok, keyword, operator).is_some()
}

/// [`editor_value`], returning the matched (lowercased) token value instead
/// of a boolean -- `EditorComment#match`'s `return value.downcase`.
fn editor_match_value(
    tok: &[u8],
    keyword: impl Fn(&[u8], usize, bool) -> Option<usize>,
    operator: u8,
) -> Option<Vec<u8>> {
    let after_kw = keyword(tok, 0, false)?;
    let i = skip_ws(tok, after_kw);
    let after_op = match_literal(tok, i, &[operator], false)?;
    let i = skip_ws(tok, after_op);
    let end = match_token(tok, i)?;
    (end == tok.len()).then(|| tok[i..end].to_ascii_lowercase())
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

/// RuboCop's `VimComment#encoding`: only present when there is more than
/// one `, `-separated token, and one of them is `fileencoding=...`.
fn vim_encoding_value(rest: &[u8]) -> Option<Vec<u8>> {
    let tokens: Vec<&[u8]> = split_seq(rest, b", ").map(trim).collect();
    if tokens.len() <= 1 {
        return None;
    }
    tokens.iter().find_map(|&tok| editor_match_value(tok, kw_fileencoding, b'='))
}

/// RuboCop's `MagicComment#encoding`, dispatched by comment shape (Emacs,
/// then Vim, then plain).
fn encoding_value(text: &[u8]) -> Option<Vec<u8>> {
    if let Some(tokens) = emacs_tokens(text) {
        return tokens.iter().find_map(|&tok| editor_match_value(tok, kw_coding, b':'));
    }
    if let Some(rest) = vim_rest(text) {
        return vim_encoding_value(rest);
    }
    simple_encoding_value(text)
}

/// RuboCop's `MagicComment#specified?`, dispatched the same way, for the
/// `:`-operator keywords every format but Vim can specify (`VimComment`
/// hard-overrides all of these to `nil`).
fn specified_colon(text: &[u8], keyword: impl Fn(&[u8], usize, bool) -> Option<usize>) -> bool {
    if let Some(tokens) = emacs_tokens(text) {
        return tokens.iter().any(|&tok| editor_value(tok, &keyword, b':'));
    }
    if vim_rest(text).is_some() {
        return false;
    }
    simple_value(text, keyword).is_some()
}

/// `Encoding.find`'s recognized names and aliases (Ruby 3.4, lower-cased),
/// consulted case-insensitively -- see `META.blind_spots`.
const KNOWN_ENCODINGS: &[&str] = &[
    "646",
    "ansi_x3.4-1968",
    "ascii",
    "ascii-8bit",
    "big5",
    "big5-hkscs",
    "big5-hkscs:2008",
    "big5-uao",
    "binary",
    "cesu-8",
    "cp1250",
    "cp1251",
    "cp1252",
    "cp1253",
    "cp1254",
    "cp1255",
    "cp1256",
    "cp1257",
    "cp1258",
    "cp437",
    "cp50220",
    "cp50221",
    "cp51932",
    "cp65000",
    "cp65001",
    "cp720",
    "cp737",
    "cp775",
    "cp850",
    "cp852",
    "cp855",
    "cp857",
    "cp860",
    "cp861",
    "cp862",
    "cp863",
    "cp864",
    "cp865",
    "cp866",
    "cp869",
    "cp874",
    "cp878",
    "cp932",
    "cp936",
    "cp949",
    "cp950",
    "cp951",
    "cswindows31j",
    "ebcdic-cp-us",
    "emacs-mule",
    "euc-cn",
    "euc-jis-2004",
    "euc-jisx0213",
    "euc-jp",
    "euc-jp-ms",
    "euc-kr",
    "euc-tw",
    "euccn",
    "eucjp",
    "eucjp-ms",
    "euckr",
    "euctw",
    "external",
    "filesystem",
    "gb12345",
    "gb18030",
    "gb1988",
    "gb2312",
    "gbk",
    "ibm037",
    "ibm437",
    "ibm720",
    "ibm737",
    "ibm775",
    "ibm850",
    "ibm852",
    "ibm855",
    "ibm857",
    "ibm860",
    "ibm861",
    "ibm862",
    "ibm863",
    "ibm864",
    "ibm865",
    "ibm866",
    "ibm869",
    "internal",
    "iso-2022-jp",
    "iso-2022-jp-2",
    "iso-2022-jp-kddi",
    "iso-8859-1",
    "iso-8859-10",
    "iso-8859-11",
    "iso-8859-13",
    "iso-8859-14",
    "iso-8859-15",
    "iso-8859-16",
    "iso-8859-2",
    "iso-8859-3",
    "iso-8859-4",
    "iso-8859-5",
    "iso-8859-6",
    "iso-8859-7",
    "iso-8859-8",
    "iso-8859-9",
    "iso2022-jp",
    "iso2022-jp2",
    "iso8859-1",
    "iso8859-10",
    "iso8859-11",
    "iso8859-13",
    "iso8859-14",
    "iso8859-15",
    "iso8859-16",
    "iso8859-2",
    "iso8859-3",
    "iso8859-4",
    "iso8859-5",
    "iso8859-6",
    "iso8859-7",
    "iso8859-8",
    "iso8859-9",
    "koi8-r",
    "koi8-u",
    "locale",
    "maccenteuro",
    "maccroatian",
    "maccyrillic",
    "macgreek",
    "maciceland",
    "macjapan",
    "macjapanese",
    "macroman",
    "macromania",
    "macthai",
    "macturkish",
    "macukraine",
    "pck",
    "shift_jis",
    "sjis",
    "sjis-docomo",
    "sjis-kddi",
    "sjis-softbank",
    "stateless-iso-2022-jp",
    "stateless-iso-2022-jp-kddi",
    "tis-620",
    "ucs-2be",
    "ucs-4be",
    "ucs-4le",
    "us-ascii",
    "utf-16",
    "utf-16be",
    "utf-16le",
    "utf-32",
    "utf-32be",
    "utf-32le",
    "utf-7",
    "utf-8",
    "utf-8-hfs",
    "utf-8-mac",
    "utf8-docomo",
    "utf8-kddi",
    "utf8-mac",
    "utf8-softbank",
    "windows-1250",
    "windows-1251",
    "windows-1252",
    "windows-1253",
    "windows-1254",
    "windows-1255",
    "windows-1256",
    "windows-1257",
    "windows-1258",
    "windows-31j",
    "windows-874",
];

/// RuboCop's `known_encoding?` (`Encoding.find(name); true rescue false`).
fn known_encoding(name: &[u8]) -> bool {
    let lower = name.to_ascii_lowercase();
    KNOWN_ENCODINGS.iter().any(|&enc| enc.as_bytes() == lower.as_slice())
}
