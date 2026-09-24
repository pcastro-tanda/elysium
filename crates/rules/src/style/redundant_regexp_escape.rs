//! `Style/RedundantRegexpEscape`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_regexp_escape.rb`.
//!
//! # Approach
//!
//! RuboCop walks `node.parsed_tree` (the `regexp_parser` gem's AST of the
//! pattern, built from the source with any `#{...}` interpolation blanked to
//! spaces of the same width, exactly like `Style/RedundantRegexpCharacterClass`)
//! tracking character-class depth via `:set` node enter/exit, and yields every
//! `:escape` node's `(char, offset, within_character_class)`. We port a small
//! purpose-built scanner over the same blanked byte buffer instead: it walks
//! byte by byte, tracking a bracket-depth counter (incremented/decremented by
//! every unescaped `[`/`]`, which -- exactly as in the sibling rule -- also
//! uniformly covers POSIX bracket expressions like `[:alnum:]` since only the
//! non-zero-ness of the depth matters here, never its exact value) and
//! extended-mode (`/x`) `#`-to-end-of-line comments (only outside a character
//! class: Ruby's free-spacing mode disables comments and whitespace
//! significance inside `[...]`), and records every backslash escape's
//! position, escaped character, and whether it fell inside a character class.
//!
//! An escape is redundant unless: its escaped character is alphanumeric
//! (`ALLOWED_ALWAYS_ESCAPES`'s single-letter-metachar carve-out, and the
//! catch-all for hex/octal/unicode/control/property escapes whose second
//! character is always a letter or digit); it is one of `ALLOWED_ALWAYS_ESCAPES`
//! (space, literal newline, `[`, `]`, `^`, `\`, `#`) or one of the regexp's own
//! delimiters (the last byte of the opening delimiter, the first byte of the
//! closing delimiter -- so `%r{...}` only exempts `{`/`}`, not `/`, while
//! `/.../ ` and `%r/.../ ` both exempt `/`); it escapes a `#`-following `@`/`$`
//! sigil to avoid accidental `#@ivar`/`#$gvar`/`#@@cvar` interpolation; or,
//! positionally, `-` within a character class (unless it begins or ends the
//! class, where the hyphen needs no escaping) or one of `.*+?{}()|$` outside
//! one. Safe autocorrect deletes just the backslash byte.
//!
//! An inner `RegularExpressionNode` reached through `#{...}` interpolation
//! (e.g. `/a#{/\-/}c/`) is a distinct AST node visited on its own by the
//! traversal, so it is handled the same way as any other regexp literal
//! without extra plumbing here.
//!
//! # Blind spots (documented in `META.blind_spots`)
//! `node.source[index]` in the original cop indexes the *whole* literal
//! (including the opening delimiter) with a content-relative offset, so its
//! `requires_escape_to_avoid_interpolation?` check is only correct for
//! single-byte delimiters (`/.../`); we instead use "the content byte right
//! before the backslash", the evidently intended semantic, which also covers
//! multi-byte `%r` delimiters RuboCop itself mishandles. No fixture exercises
//! that RuboCop discrepancy.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG_REDUNDANT_ESCAPE`.
const MSG: &str = "Redundant escape inside regexp literal";

/// Checks for redundant escapes inside `Regexp` literals.
#[derive(Debug, Clone, Default)]
pub struct RedundantRegexpEscape;

impl Rule for RedundantRegexpEscape {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantRegexpEscape",
        department: Department::Style,
        summary: "Checks for redundant escapes inside `Regexp` literals.",
        explanation: "\
```ruby
# bad
%r{foo\\/bar}

# good
%r{foo/bar}

# good
/foo\\/bar/

# good
%r/foo\\/bar/

# good
%r!foo\\!bar!

# bad
/a\\-b/

# good
/a-b/

# bad
/[\\+\\-]\\d/

# good
/[+\\-]\\d/
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::RegularExpressionNode, NodeKind::InterpolatedRegularExpressionNode],
        config: &[],
        blind_spots: "\
`node.source[index]` in the original cop indexes the whole literal (opening \
delimiter included) with a content-relative offset, so its interpolation- \
sigil check is only correct for single-byte delimiters (`/.../`); we use \
\"the content byte right before the backslash\" instead, which is the \
evidently intended semantic and also covers multi-byte `%r` delimiters \
RuboCop itself mishandles. No known fixture exercises that discrepancy.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::RegularExpressionNode => {
                let Some(n) = node.as_regular_expression_node() else { return };
                let span = n.content_loc().span();
                let buf = ctx.text(span);
                let delimiters =
                    node_delimiters(ctx, n.opening_loc().span(), n.closing_loc().span());
                scan_and_report(ctx, span.start, buf, n.is_extended(), delimiters);
            }
            NodeKind::InterpolatedRegularExpressionNode => {
                let Some(n) = node.as_interpolated_regular_expression_node() else { return };
                let parts = n.parts();
                let mut buf: Vec<u8> = Vec::new();
                let mut base: Option<u32> = None;
                for part in &parts {
                    let pspan = part.span();
                    if base.is_none() {
                        base = Some(pspan.start);
                    }
                    if part.kind() == NodeKind::StringNode {
                        buf.extend_from_slice(ctx.text(pspan));
                    } else {
                        buf.resize(buf.len() + pspan.len() as usize, b' ');
                    }
                }
                let Some(base) = base else { return };
                let delimiters =
                    node_delimiters(ctx, n.opening_loc().span(), n.closing_loc().span());
                scan_and_report(ctx, base, &buf, n.is_extended(), delimiters);
            }
            _ => {}
        }
    }
}

/// The regexp's delimiter bytes: the last byte of the opening delimiter, the
/// first byte of the closing delimiter (RuboCop's `RegexpNode#delimiters`).
fn node_delimiters(ctx: &Context<'_>, opening: Span, closing: Span) -> (u8, u8) {
    let open = ctx.text(opening);
    let close = ctx.text(closing);
    (*open.last().unwrap_or(&0), *close.first().unwrap_or(&0))
}

/// One backslash escape found in a regexp's (interpolation-blanked) content.
struct Escape {
    /// Byte offset of the backslash, relative to `buf`.
    pos: usize,
    /// Byte offset of the escaped character, relative to `buf`.
    char_pos: usize,
    /// Byte length of the escaped character.
    char_len: usize,
    /// Whether the escape falls inside an (unescaped) `[...]` character class.
    within_class: bool,
}

/// Scans `buf` (the regexp's content, source bytes with any interpolation
/// blanked to spaces of the same width) for redundant escapes, reporting
/// each one found with a safe fix that deletes just the backslash. `base` is
/// `buf`'s starting byte offset in the file.
#[allow(clippy::cast_possible_truncation)]
fn scan_and_report(
    ctx: &mut Context<'_>,
    base: u32,
    buf: &[u8],
    extended: bool,
    delimiters: (u8, u8),
) {
    for escape in scan_escapes(buf, extended) {
        if allowed_escape(buf, &escape, delimiters) {
            continue;
        }
        let span =
            Span::new(base + escape.pos as u32, base + (escape.char_pos + escape.char_len) as u32);
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::delete(Span::new(
                base + escape.pos as u32,
                base + escape.pos as u32 + 1,
            ))],
        };
        ctx.report_with_fix(&RedundantRegexpEscape::META, span, MSG, fix);
    }
}

/// Scans `buf` for every backslash escape, tracking character-class depth
/// (any unescaped `[`/`]`, matching POSIX bracket expressions along with
/// real/nested classes -- only whether depth is non-zero matters here) and,
/// outside a class in extended mode, skipping `#`-to-end-of-line comments.
fn scan_escapes(buf: &[u8], extended: bool) -> Vec<Escape> {
    let len = buf.len();
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut depth: u32 = 0;
    while pos < len {
        let c = buf[pos];
        if c == b'\\' {
            let char_pos = pos + 1;
            if char_pos >= len {
                break;
            }
            let char_len = utf8_len(buf, char_pos);
            out.push(Escape { pos, char_pos, char_len, within_class: depth > 0 });
            pos = char_pos + char_len;
            continue;
        }
        if depth == 0 && extended && c == b'#' {
            while pos < len && buf[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        if c == b'[' {
            depth += 1;
            pos += 1;
            continue;
        }
        if c == b']' && depth > 0 {
            depth -= 1;
            pos += 1;
            continue;
        }
        pos += utf8_len(buf, pos);
    }
    out
}

/// RuboCop's `allowed_escape?`.
fn allowed_escape(buf: &[u8], escape: &Escape, delimiters: (u8, u8)) -> bool {
    let bytes = &buf[escape.char_pos..escape.char_pos + escape.char_len];
    let Some(ch) = std::str::from_utf8(bytes).ok().and_then(|s| s.chars().next()) else {
        // Invalid encoding: RuboCop's `char.valid_encoding?` guard fails, so
        // the escape is never treated as allowed.
        return false;
    };
    if ch.is_alphanumeric() {
        return true;
    }
    if matches!(ch, ' ' | '\n' | '[' | ']' | '^' | '\\' | '#') {
        return true;
    }
    if bytes[0] == delimiters.0 || bytes[0] == delimiters.1 {
        return true;
    }
    if requires_escape_to_avoid_interpolation(buf, escape.pos, ch) {
        return true;
    }
    if escape.within_class {
        ch == '-' && !hyphen_begins_or_ends_class(buf, escape.pos)
    } else {
        matches!(ch, '.' | '*' | '+' | '?' | '{' | '}' | '(' | ')' | '|' | '$')
    }
}

/// RuboCop's `requires_escape_to_avoid_interpolation?`: preserves escapes
/// after a literal `#` that would otherwise trigger `#@ivar`/`#$gvar`/
/// `#@@cvar` interpolation.
fn requires_escape_to_avoid_interpolation(buf: &[u8], pos: usize, escaped: char) -> bool {
    pos > 0 && buf[pos - 1] == b'#' && matches!(escaped, '@' | '$')
}

/// RuboCop's `char_class_begins_or_ends_with_escaped_hyphen?`: a `-` escape
/// needs no escaping (and so is flagged) when it is the first or last
/// element of its character class.
fn hyphen_begins_or_ends_class(buf: &[u8], pos: usize) -> bool {
    if buf.get(pos + 2) == Some(&b']') {
        return true;
    }
    if pos >= 1 && buf.get(pos - 1) == Some(&b'[') {
        return pos < 2 || buf.get(pos - 2) != Some(&b'\\');
    }
    false
}

/// The byte length of the UTF-8 sequence starting at `buf[pos]`, clamped to
/// the buffer's remaining length; `1` past the end or for a lone
/// continuation/invalid leading byte.
fn utf8_len(buf: &[u8], pos: usize) -> usize {
    if pos >= buf.len() {
        return 1;
    }
    let b = buf[pos];
    let want = if b & 0x80 == 0 {
        1
    } else if b & 0xE0 == 0xC0 {
        2
    } else if b & 0xF0 == 0xE0 {
        3
    } else if b & 0xF8 == 0xF0 {
        4
    } else {
        1
    };
    want.min(buf.len() - pos)
}
