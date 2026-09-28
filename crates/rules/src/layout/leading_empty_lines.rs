//! `Layout/LeadingEmptyLines`, ported from RuboCop's
//! `lib/rubocop/cop/layout/leading_empty_lines.rb`.
//!
//! Upstream flags the file when `processed_source.tokens[0]` (the very
//! first lexical token, comment or code) starts on a line after 1, and
//! removes everything before that token's start. This port finds the same
//! byte offset directly (the first non-whitespace byte) and, for the
//! offense's highlighted range, either reuses the first parsed comment (if
//! the content there is a comment) or re-derives the width of the first
//! lexical token with a small scanner, since this engine does not keep a
//! token stream.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Unnecessary blank line at the beginning of the source.";

/// Checks for unnecessary leading blank lines at the beginning of a file.
#[derive(Debug, Clone, Default)]
pub struct LeadingEmptyLines;

impl Rule for LeadingEmptyLines {
    const META: RuleMeta = RuleMeta {
        name: "Layout/LeadingEmptyLines",
        department: Department::Layout,
        summary: "Checks for unnecessary blank lines at the beginning of a file.",
        explanation: "\
```ruby
# bad
# (start of file)

class Foo
end

# bad
# (start of file)

# a comment

# good
# (start of file)
class Foo
end

# good
# (start of file)
# a comment
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
        let source = ctx.source().bytes();
        let Some(start) = source.iter().position(|&b| !is_ws(b)) else { return };
        let start = u32::try_from(start).expect("offset exceeds u32");
        if ctx.line_col(start).line <= 1 {
            return;
        }

        let width = ctx
            .comments()
            .first()
            .filter(|c| c.span.start == start)
            .map_or_else(|| first_token_width(source, start), |c| c.span.end - start);
        let range = Span::new(start, start + width);

        ctx.report_with_fix(
            &Self::META,
            range,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::delete(Span::new(0, start))],
            },
        );
    }
}

/// Ruby's lexer whitespace: space, tab, newline, vertical tab, form feed,
/// carriage return. Matches how the real tokenizer skips bytes between
/// tokens (and, since this only runs before any token has been seen, is
/// exactly RuboCop's `processed_source.tokens[0]` search).
fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

/// Multi-character operator/punctuator tokens, longest first, tried against
/// the byte fallback path. Single-character tokens (brackets, commas, most
/// operators) fall through to the one-byte default.
const MULTI_CHAR_OPS: &[&str] = &[
    "<=>", "===", "**=", "...", "&&=", "||=", "<<=", ">>=", "!~", "=~", "==", "!=", "<=", ">=",
    "&&", "||", "**", "<<", ">>", "..", "::", "->", "=>", "+@", "-@",
];

/// A very small re-derivation of "the first lexical token's width" for the
/// byte at `start` in `source`, covering the token shapes that can occur at
/// the top of a real Ruby file (keywords/identifiers, ivars/cvars/gvars,
/// numbers, symbols, strings, percent literals, heredoc openers) plus a
/// generic operator/punctuation fallback. Comments are handled by the
/// caller via [`Context::comments`], not here.
fn first_token_width(source: &[u8], start: u32) -> u32 {
    let bytes = &source[start as usize..];
    let len = bytes.len();
    let b0 = bytes[0];

    if is_ident_start(b0) {
        return u32::try_from(ident_width(bytes)).expect("offset exceeds u32");
    }
    if b0 == b'@' {
        let mut i = 1;
        if i < len && bytes[i] == b'@' {
            i += 1;
        }
        while i < len && is_ident_continue(bytes[i]) {
            i += 1;
        }
        return u32::try_from(i).expect("offset exceeds u32");
    }
    if b0 == b'$' {
        let mut i = 1;
        if i < len && is_ident_continue(bytes[i]) {
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
        } else if i < len {
            i += 1;
        }
        return u32::try_from(i).expect("offset exceeds u32");
    }
    if b0.is_ascii_digit() {
        return u32::try_from(number_width(bytes)).expect("offset exceeds u32");
    }
    if b0 == b':' && bytes.get(1) != Some(&b':') {
        return u32::try_from(symbol_width(bytes)).expect("offset exceeds u32");
    }
    if b0 == b'"' || b0 == b'\'' {
        return u32::try_from(string_width(bytes)).expect("offset exceeds u32");
    }
    if b0 == b'%' {
        if let Some(w) = percent_width(bytes) {
            return u32::try_from(w).expect("offset exceeds u32");
        }
    }
    if b0 == b'<' && bytes.get(1) == Some(&b'<') {
        if let Some(w) = heredoc_width(bytes) {
            return u32::try_from(w).expect("offset exceeds u32");
        }
    }
    for op in MULTI_CHAR_OPS {
        if bytes.starts_with(op.as_bytes()) {
            return u32::try_from(op.len()).expect("offset exceeds u32");
        }
    }
    1
}

/// `[A-Za-z_]\w*` (Prism/Ruby identifiers, ASCII-approximated for
/// multi-byte UTF-8 letters) with an optional trailing `?`/`!`, unless it
/// would instead read as the start of a `!=` operator.
fn ident_width(bytes: &[u8]) -> usize {
    let mut i = 0;
    while i < bytes.len() && is_ident_continue(bytes[i]) {
        i += 1;
    }
    if i < bytes.len() && matches!(bytes[i], b'?' | b'!') && bytes.get(i + 1) != Some(&b'=') {
        i += 1;
    }
    i
}

/// Ruby numeric literal: `0x`/`0b`/`0o`/`0d` radix prefixes, or decimal with
/// an optional fractional part and exponent, plus an optional trailing
/// `r`/`i` (rational/imaginary) suffix.
fn number_width(bytes: &[u8]) -> usize {
    let len = bytes.len();
    let mut i = 1;
    if bytes[0] == b'0'
        && matches!(bytes.get(1), Some(b'x' | b'X' | b'b' | b'B' | b'o' | b'O' | b'd' | b'D'))
    {
        i = 2;
        while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
    } else {
        while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            i += 1;
        }
        if bytes.get(i) == Some(&b'.') && bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
            i += 1;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
            }
        }
        if matches!(bytes.get(i), Some(b'e' | b'E')) {
            let mut j = i + 1;
            if matches!(bytes.get(j), Some(b'+' | b'-')) {
                j += 1;
            }
            if bytes.get(j).is_some_and(u8::is_ascii_digit) {
                i = j;
                while i < len && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
        }
    }
    if matches!(bytes.get(i), Some(b'r' | b'i')) {
        i += 1;
    }
    i
}

/// Named ("`op`") symbol bodies, longest first: covers the common operator
/// method names (`:[]`, `:<=>`, `:+@`, ...).
const SYMBOL_OPS: &[&str] = &[
    "<=>", "===", "[]=", "**", "==", "!=", "<=", ">=", "<<", ">>", "=~", "!~", "[]", "+@", "-@",
    "::", "..",
];

/// `:name`, `:"..."`/`:'...'` (just the two-byte opener; the quoted body is
/// tokenized separately upstream), or `:<op>` for a symbol naming an
/// operator method.
fn symbol_width(bytes: &[u8]) -> usize {
    let rest = &bytes[1..];
    if matches!(rest.first(), Some(b'"' | b'\'')) {
        return 2;
    }
    if rest.first().is_some_and(|&b| is_ident_start(b)) {
        return 1 + ident_width(rest);
    }
    for op in SYMBOL_OPS {
        if rest.starts_with(op.as_bytes()) {
            return 1 + op.len();
        }
    }
    if rest.is_empty() {
        1
    } else {
        2
    }
}

/// A quoted string literal. A single- or double-quoted literal with no
/// interpolation before its closing quote is one token spanning the whole
/// literal; a double-quoted literal that reaches `#{` first is only its
/// opening quote (the rest lexes as separate string-content/embexpr
/// tokens upstream).
fn string_width(bytes: &[u8]) -> usize {
    let quote = bytes[0];
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b if b == quote => return i + 1,
            b'#' if quote == b'"' && bytes.get(i + 1) == Some(&b'{') => return 1,
            _ => i += 1,
        }
    }
    i.min(bytes.len())
}

/// `%w[`, `%i(`, `%q{`, `%(`, ... : `%` plus an optional type letter plus
/// the delimiter byte. Returns `None` when `bytes` does not start a percent
/// literal (e.g. plain `%` or `%=` modulo).
fn percent_width(bytes: &[u8]) -> Option<usize> {
    let mut i = 1;
    if matches!(bytes.get(1), Some(b'q' | b'Q' | b'w' | b'W' | b'i' | b'I' | b'r' | b's')) {
        i = 2;
    }
    let delim = *bytes.get(i)?;
    if delim.is_ascii_alphanumeric() || delim == b' ' || delim == b'=' {
        return None;
    }
    Some(i + 1)
}

/// `<<~TAG`, `<<-TAG`, `<<TAG`, or the quoted-tag forms `<<~"TAG"`, etc.:
/// the whole opener, matching upstream's single `tSTRING_BEG` token for a
/// heredoc. Returns `None` when `bytes` does not start a heredoc (so the
/// caller falls back to treating `<<` as an operator).
fn heredoc_width(bytes: &[u8]) -> Option<usize> {
    let mut i = 2;
    if matches!(bytes.get(i), Some(b'~' | b'-')) {
        i += 1;
    }
    match bytes.get(i) {
        Some(&q @ (b'"' | b'\'' | b'`')) => {
            let tag_start = i + 1;
            let mut j = tag_start;
            while j < bytes.len() && bytes[j] != q {
                j += 1;
            }
            if j >= bytes.len() || j == tag_start {
                return None;
            }
            Some(j + 1)
        }
        Some(&b) if is_ident_start(b) => {
            let tag_start = i;
            let mut j = tag_start;
            while j < bytes.len() && is_ident_continue(bytes[j]) {
                j += 1;
            }
            Some(j)
        }
        _ => None,
    }
}
