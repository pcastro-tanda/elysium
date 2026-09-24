//! `Style/RedundantRegexpCharacterClass`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_regexp_character_class.rb`.
//!
//! # Approach
//!
//! RuboCop parses the regexp's literal source with the `regexp_parser` gem
//! (`node.parsed_tree`), blanking out any `#{...}` interpolation to spaces
//! of the same width first (`with_interpolations_blanked`) so byte offsets
//! still line up with the original source. We port a small, purpose-built
//! scanner over the same blanked byte buffer instead of a full regex
//! grammar: it tracks backslash escapes, `/x`-mode comments, and bracket
//! nesting (which uniformly covers both nested character sets and POSIX
//! bracket expressions like `[:alnum:]`, since both are simply a balanced
//! `[`...`]` run) to find each top-level `[...]` character class and its
//! element(s).
//!
//! A character class is redundant when it contains exactly one element
//! (never a range, POSIX class, nested set, or set-intersection -- any of
//! those make a class with two or more raw tokens, or are themselves
//! excluded types, in `regexp_parser`'s tree, which our scanner mirrors by
//! simply requiring exactly one scanned element), it isn't negated, and
//! that element isn't `\u{XX YY}`-style multiple-codepoint escape,
//! whitespace under `/x` mode, `\b` (backspace only inside a class), a
//! bare `\1`-`\7` octal backreference digit (only unambiguous inside a
//! class), or one of the chars that require escaping outside a class
//! (`.*+?{}()|$`).
//!
//! An inner `RegularExpressionNode` reached through `#{...}` interpolation
//! (e.g. `/a#{/[b]/}c/`) is a distinct AST node visited on its own by the
//! traversal, so it is handled the same way as any other regexp literal
//! without extra plumbing here.
//!
//! # Blind spots (documented in `META.blind_spots`)
//! `\c`/`\C-`/`\M-` control/meta escapes are consumed but not validated;
//! `[]...]`/`[^]...]` (leading literal `]` right after the opening
//! bracket) is not special-cased, matching a rare Oniguruma idiom RuboCop's
//! own spec suite does not exercise either.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG_REDUNDANT_CHARACTER_CLASS`.
const MSG: &str =
    "Redundant single-element character class, `{class}` can be replaced with `{element}`.";

/// RuboCop's `REQUIRES_ESCAPE_OUTSIDE_CHAR_CLASS_CHARS`.
const REQUIRES_ESCAPE_OUTSIDE_CHAR_CLASS: &[u8] = b".*+?{}()|$";

/// Looks for unnecessary single-element `Regexp` character classes.
#[derive(Debug, Clone, Default)]
pub struct RedundantRegexpCharacterClass;

impl Rule for RedundantRegexpCharacterClass {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantRegexpCharacterClass",
        department: Department::Style,
        summary: "Checks for unnecessary single-element `Regexp` character classes.",
        explanation: "\
```ruby
# bad
r = /[x]/

# good
r = /x/

# bad
r = /[\\s]/

# good
r = /\\s/

# bad
r = %r{/[b]}

# good
r = %r{/b}

# good
r = /[ab]/
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::RegularExpressionNode, NodeKind::InterpolatedRegularExpressionNode],
        config: &[],
        blind_spots: "\
`\\c`/`\\C-`/`\\M-` control and meta escapes are consumed as opaque single \
elements (their internal validity is never checked, only their extent). \
Leading `]` right after `[`/`[^]` (a rare Oniguruma idiom for a class that \
contains a literal `]`) is not special-cased and is treated as an ordinary \
close bracket, matching RuboCop's own spec suite which does not exercise \
it either.",
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
                scan_content(ctx, span.start, buf, n.is_extended());
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
                if let Some(base) = base {
                    scan_content(ctx, base, &buf, n.is_extended());
                }
            }
            _ => {}
        }
    }
}

/// One element found inside a top-level character class: either literal
/// text (a plain character or an escape sequence, with `multi_codepoint`
/// set for `\u{XX YY}`/`\x{XX YY}`-style multi-codepoint escapes), or an
/// excluded token (a nested set, POSIX class, or set-intersection --
/// anything itself delimited by a balanced `[`...`]` run, which
/// `regexp_parser` always types as `:set`/`:posixclass`/`:nonposixclass`).
enum Element {
    Text { bytes: Vec<u8>, multi_codepoint: bool },
    Excluded,
}

/// Scans `buf` (the regexp's literal content, source bytes with any
/// interpolation blanked to spaces of the same width) for redundant
/// single-element character classes, reporting each one found. `base` is
/// `buf`'s starting byte offset in the file.
fn scan_content(ctx: &mut Context<'_>, base: u32, buf: &[u8], extended: bool) {
    let len = buf.len();
    let mut pos = 0usize;
    while pos < len {
        let c = buf[pos];
        if c == b'\\' {
            pos += 1 + utf8_len(buf, pos + 1);
            continue;
        }
        if extended && c == b'#' {
            while pos < len && buf[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        if c == b'[' {
            let class_start = pos;
            let (negated, elements, close_pos) = parse_char_class(buf, pos, len);
            pos = close_pos;
            if !negated {
                if let [Element::Text { bytes, multi_codepoint: false }] = elements.as_slice() {
                    if !is_non_redundant(bytes, extended) {
                        report(ctx, base, class_start, close_pos, buf, bytes);
                    }
                }
            }
            continue;
        }
        pos += utf8_len(buf, pos);
    }
}

/// Parses a character class starting at `buf[pos] == '['`. Returns whether
/// it is negated (`[^...]`), its elements, and the position just past the
/// closing `]`.
fn parse_char_class(buf: &[u8], pos: usize, len: usize) -> (bool, Vec<Element>, usize) {
    let mut p = pos + 1;
    let mut negated = false;
    if p < len && buf[p] == b'^' {
        negated = true;
        p += 1;
    }
    let mut elements = Vec::new();
    while p < len {
        let c = buf[p];
        if c == b']' {
            return (negated, elements, p + 1);
        }
        if c == b'[' {
            let mut depth = 1usize;
            p += 1;
            while depth > 0 && p < len {
                match buf[p] {
                    b'\\' => p += 1 + utf8_len(buf, p + 1),
                    b'[' => {
                        depth += 1;
                        p += 1;
                    }
                    b']' => {
                        depth -= 1;
                        p += 1;
                    }
                    _ => p += utf8_len(buf, p),
                }
            }
            elements.push(Element::Excluded);
            continue;
        }
        if c == b'\\' {
            let (bytes, newpos, multi_codepoint) = parse_escape(buf, p, len);
            elements.push(Element::Text { bytes, multi_codepoint });
            p = newpos;
            continue;
        }
        let clen = utf8_len(buf, p);
        elements.push(Element::Text { bytes: buf[p..p + clen].to_vec(), multi_codepoint: false });
        p += clen;
    }
    // Unterminated (would be a Prism parse error in practice); stop where we ran out.
    (negated, elements, p)
}

/// Parses one backslash escape starting at `buf[pos] == '\\'`. Returns the
/// escape's raw text (including the backslash), the position just past it,
/// and whether it is a `\u{}`/`\x{}` escape naming two or more codepoints.
fn parse_escape(buf: &[u8], pos: usize, len: usize) -> (Vec<u8>, usize, bool) {
    let start = pos;
    let p = pos + 1;
    if p >= len {
        return (buf[start..p].to_vec(), p, false);
    }
    match buf[p] {
        b'u' | b'x' if p + 1 < len && buf[p + 1] == b'{' => {
            let (end, multi) = scan_braced_codepoints(buf, p + 2, len);
            (buf[start..end].to_vec(), end, multi)
        }
        b'u' => {
            let end = scan_hex_digits(buf, p + 1, len, 4);
            (buf[start..end].to_vec(), end, false)
        }
        b'x' => {
            let end = scan_hex_digits(buf, p + 1, len, 2);
            (buf[start..end].to_vec(), end, false)
        }
        b'p' | b'P' if p + 1 < len && buf[p + 1] == b'{' => {
            let mut q = p + 2;
            while q < len && buf[q] != b'}' {
                q += 1;
            }
            let end = if q < len { q + 1 } else { q };
            (buf[start..end].to_vec(), end, false)
        }
        b'0'..=b'7' => {
            let mut q = p;
            let mut n = 0;
            while q < len && n < 3 && buf[q].is_ascii_digit() && buf[q] < b'8' {
                q += 1;
                n += 1;
            }
            (buf[start..q].to_vec(), q, false)
        }
        b'c' | b'C' | b'M' => {
            let mut q = p + 1;
            if buf[p] != b'c' && q < len && buf[q] == b'-' {
                q += 1;
            }
            if q < len {
                q += if buf[q] == b'\\' { 1 + utf8_len(buf, q + 1) } else { utf8_len(buf, q) };
            }
            (buf[start..q].to_vec(), q, false)
        }
        _ => {
            let clen = utf8_len(buf, p);
            (buf[start..p + clen].to_vec(), p + clen, false)
        }
    }
}

/// Scans a `\u{...}`/`\x{...}` codepoint list starting just after the `{`.
/// Returns the position just past the closing `}` and whether it names two
/// or more whitespace-separated codepoints.
fn scan_braced_codepoints(buf: &[u8], start: usize, len: usize) -> (usize, bool) {
    let mut q = start;
    while q < len && buf[q] != b'}' {
        q += 1;
    }
    let inner = &buf[start..q];
    let codepoints =
        inner.split(|b: &u8| b.is_ascii_whitespace()).filter(|s| !s.is_empty()).count();
    let end = if q < len { q + 1 } else { q };
    (end, codepoints >= 2)
}

/// Scans up to `max` hex digits starting at `start`, returning the
/// position just past the last one consumed.
fn scan_hex_digits(buf: &[u8], start: usize, len: usize, max: usize) -> usize {
    let mut q = start;
    let mut n = 0;
    while q < len && n < max && buf[q].is_ascii_hexdigit() {
        q += 1;
        n += 1;
    }
    q
}

/// The byte length of the UTF-8 sequence starting at `buf[pos]`, clamped
/// to the buffer's remaining length; `1` past the end or for a lone
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

/// RuboCop's `redundant_single_element_character_class?`, inverted: true
/// when unwrapping `elem` from its character class would change meaning.
fn is_non_redundant(elem: &[u8], extended: bool) -> bool {
    (extended && elem.iter().any(|&b| is_ruby_whitespace(b)))
        || elem == b"\\b"
        || (elem.len() == 2 && elem[0] == b'\\' && (b'1'..=b'7').contains(&elem[1]))
        || (elem.len() == 1 && REQUIRES_ESCAPE_OUTSIDE_CHAR_CLASS.contains(&elem[0]))
}

/// Ruby's default (non-Unicode) `\s`: space, tab, newline, vertical tab,
/// form feed, carriage return.
fn is_ruby_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

/// Reports one redundant single-element character class and its safe fix.
#[allow(clippy::cast_possible_truncation)]
fn report(
    ctx: &mut Context<'_>,
    base: u32,
    class_start: usize,
    close_pos: usize,
    buf: &[u8],
    elem: &[u8],
) {
    let span = Span::new(base + class_start as u32, base + close_pos as u32);
    let class_bytes = &buf[class_start..close_pos];
    let replacement: Vec<u8> = if class_bytes == b"[#]" {
        let mut v = Vec::with_capacity(elem.len() + 1);
        v.push(b'\\');
        v.extend_from_slice(elem);
        v
    } else {
        elem.to_vec()
    };
    let message = MSG.replacen("{class}", &String::from_utf8_lossy(class_bytes), 1).replacen(
        "{element}",
        &String::from_utf8_lossy(&replacement),
        1,
    );
    let fix =
        Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(span, replacement)] };
    ctx.report_with_fix(&RedundantRegexpCharacterClass::META, span, message, fix);
}
