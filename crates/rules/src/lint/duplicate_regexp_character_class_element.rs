//! `Lint/DuplicateRegexpCharacterClassElement`, ported from RuboCop's
//! `lib/rubocop/cop/lint/duplicate_regexp_character_class_element.rb`.
//!
//! # Approach
//!
//! RuboCop parses the regexp's literal source with the `regexp_parser` gem
//! (`node.parsed_tree`, built from the source with any `#{...}`
//! interpolation blanked to spaces of the same width first, so byte offsets
//! still line up with the original source) and, for every top-level
//! character-class expression (`expr.type == :set`, skipping any class
//! containing a top-level `&&` set-intersection operator entirely), walks
//! its immediate elements, reporting each one whose own source text repeats
//! an earlier sibling's (skipping elements that fall inside a blanked
//! interpolation). This port uses the same purpose-built byte scanner as
//! `Style::RedundantRegexpCharacterClass` (copied privately here, per the
//! porting kit) for backslash escapes and balanced-bracket nested/POSIX
//! classes (`[:alnum:]`, always consumed as one atomic, "excluded-from-
//! recursion" element, matching `regexp_parser` typing it `:posixclass`
//! rather than `:set` and thus never separately re-processed by
//! `skip_expression?`), extended with a second pass that merges a
//! `literal`-`-`-`literal` run into one `Range`-like element (`regexp_parser`
//! groups a range as a single expression) before the duplicate scan.
//!
//! # Blind spots (documented in `META.blind_spots`)
//! `\c`/`\C-`/`\M-` control and meta escapes are consumed as opaque single
//! elements (their internal validity is never checked, only their extent).
//! A character class whose own elements include a genuinely nested literal
//! set (Oniguruma's `[a[bc]]` union form, rather than a POSIX bracket
//! expression) is scanned as one atomic, never-recursed-into element at the
//! outer level, instead of being separately walked for its own internal
//! duplicates the way `regexp_parser`'s tree-wide `each_expression` would;
//! no known fixture uses that rare form. Leading `]` right after `[`/`[^]`
//! (a rare Oniguruma idiom for a class that contains a literal `]`) is not
//! special-cased and is treated as an ordinary close bracket, matching
//! RuboCop's own spec suite which does not exercise it either.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG_REPEATED_ELEMENT`.
const MSG: &str = "Duplicate element inside regexp character class";

/// Checks for duplicate elements in `Regexp` character classes.
#[derive(Debug, Clone, Default)]
pub struct DuplicateRegexpCharacterClassElement;

impl Rule for DuplicateRegexpCharacterClassElement {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DuplicateRegexpCharacterClassElement",
        department: Department::Lint,
        summary: "Checks for duplicate elements in Regexp character classes.",
        explanation: "\
Checks for duplicate elements in `Regexp` character classes.

```ruby
# bad
r = /[xyx]/

# bad
r = /[0-9x0-9]/

# good
r = /[xy]/

# good
r = /[0-9x]/
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::RegularExpressionNode, NodeKind::InterpolatedRegularExpressionNode],
        config: &[],
        blind_spots: "\
`\\c`/`\\C-`/`\\M-` control and meta escapes are consumed as opaque single \
elements without validating their internal syntax. A genuinely nested \
literal set (Oniguruma's `[a[bc]]` union form, as opposed to a POSIX \
bracket expression) is scanned as one atomic element at the outer level \
rather than separately walked for its own internal duplicates; no known \
fixture uses that rare form. Leading `]` right after `[`/`[^]` (a rare \
Oniguruma idiom for a class that contains a literal `]`) is treated as an \
ordinary close bracket, matching RuboCop's own spec suite.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::RegularExpressionNode => {
                let Some(n) = node.as_regular_expression_node() else { return };
                let span = n.content_loc().span();
                let buf = ctx.text(span).to_vec();
                scan_content(ctx, span.start, &buf, &[]);
            }
            NodeKind::InterpolatedRegularExpressionNode => {
                let Some(n) = node.as_interpolated_regular_expression_node() else { return };
                let parts = n.parts();
                let mut buf: Vec<u8> = Vec::new();
                let mut base: Option<u32> = None;
                let mut interpolations: Vec<Span> = Vec::new();
                for part in &parts {
                    let pspan = part.span();
                    if base.is_none() {
                        base = Some(pspan.start);
                    }
                    if part.kind() == NodeKind::StringNode {
                        buf.extend_from_slice(ctx.text(pspan));
                    } else {
                        interpolations.push(pspan);
                        buf.resize(buf.len() + pspan.len() as usize, b' ');
                    }
                }
                if let Some(base) = base {
                    scan_content(ctx, base, &buf, &interpolations);
                }
            }
            _ => {}
        }
    }
}

/// One element found inside a top-level character class, before range
/// merging: a literal atom (a plain character or an escape sequence) or an
/// excluded token (a nested set or POSIX class -- anything itself delimited
/// by a balanced `[`...`]` run).
struct RawElement {
    start: usize,
    end: usize,
    excluded: bool,
}

/// Scans `buf` (the regexp's literal content, with any interpolation
/// blanked to spaces of the same width) for top-level character classes,
/// reporting duplicate elements in each one that has no top-level `&&`
/// set-intersection operator. `base` is `buf`'s starting byte offset in the
/// file; `interpolations` are the file byte spans the blanking replaced.
fn scan_content(ctx: &mut Context<'_>, base: u32, buf: &[u8], interpolations: &[Span]) {
    let len = buf.len();
    let mut pos = 0usize;
    while pos < len {
        let c = buf[pos];
        if c == b'\\' {
            pos += 1 + utf8_len(buf, pos + 1);
            continue;
        }
        if c == b'[' {
            let (raw, has_intersection, close_pos) = parse_char_class(buf, pos, len);
            if !has_intersection {
                let elements = merge_ranges(buf, &raw);
                report_duplicates(ctx, base, buf, &elements, interpolations);
            }
            pos = close_pos;
            continue;
        }
        pos += utf8_len(buf, pos);
    }
}

/// Parses a character class starting at `buf[pos] == '['`. Returns its raw
/// (pre-range-merge) elements, whether it contains a top-level `&&`
/// set-intersection operator, and the position just past the closing `]`.
fn parse_char_class(buf: &[u8], pos: usize, len: usize) -> (Vec<RawElement>, bool, usize) {
    let mut p = pos + 1;
    if p < len && buf[p] == b'^' {
        p += 1;
    }
    let mut elements = Vec::new();
    let mut has_intersection = false;
    while p < len {
        let c = buf[p];
        if c == b']' {
            return (elements, has_intersection, p + 1);
        }
        if c == b'&' && p + 1 < len && buf[p + 1] == b'&' {
            has_intersection = true;
            p += 2;
            continue;
        }
        if c == b'[' {
            let start = p;
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
            elements.push(RawElement { start, end: p, excluded: true });
            continue;
        }
        if c == b'\\' {
            let start = p;
            let newpos = parse_escape_end(buf, p, len);
            elements.push(RawElement { start, end: newpos, excluded: false });
            p = newpos;
            continue;
        }
        let clen = utf8_len(buf, p);
        elements.push(RawElement { start: p, end: p + clen, excluded: false });
        p += clen;
    }
    // Unterminated (would be a Prism parse error in practice); stop where we ran out.
    (elements, has_intersection, p)
}

/// Parses one backslash escape starting at `buf[pos] == '\\'`. Returns the
/// position just past it.
fn parse_escape_end(buf: &[u8], pos: usize, len: usize) -> usize {
    let p = pos + 1;
    if p >= len {
        return p;
    }
    match buf[p] {
        b'u' | b'x' if p + 1 < len && buf[p + 1] == b'{' => scan_braced(buf, p + 2, len),
        b'u' => scan_hex_digits(buf, p + 1, len, 4),
        b'x' => scan_hex_digits(buf, p + 1, len, 2),
        b'p' | b'P' if p + 1 < len && buf[p + 1] == b'{' => {
            let mut q = p + 2;
            while q < len && buf[q] != b'}' {
                q += 1;
            }
            if q < len {
                q + 1
            } else {
                q
            }
        }
        b'0'..=b'7' => {
            let mut q = p;
            let mut n = 0;
            while q < len && n < 3 && buf[q].is_ascii_digit() && buf[q] < b'8' {
                q += 1;
                n += 1;
            }
            q
        }
        b'c' | b'C' | b'M' => {
            let mut q = p + 1;
            if buf[p] != b'c' && q < len && buf[q] == b'-' {
                q += 1;
            }
            if q < len {
                q += if buf[q] == b'\\' { 1 + utf8_len(buf, q + 1) } else { utf8_len(buf, q) };
            }
            q
        }
        _ => {
            let clen = utf8_len(buf, p);
            p + clen
        }
    }
}

/// Scans a `\u{...}`/`\x{...}` codepoint list starting just after the `{`.
/// Returns the position just past the closing `}`.
fn scan_braced(buf: &[u8], start: usize, len: usize) -> usize {
    let mut q = start;
    while q < len && buf[q] != b'}' {
        q += 1;
    }
    if q < len {
        q + 1
    } else {
        q
    }
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

/// Merges a `literal`-`-`-`literal` run of raw elements into a single
/// range-like element, matching `regexp_parser` grouping `a-c` as one
/// `Range` expression rather than three siblings.
fn merge_ranges(buf: &[u8], raw: &[RawElement]) -> Vec<RawElement> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if i + 2 < raw.len() {
            let (a, dash, b) = (&raw[i], &raw[i + 1], &raw[i + 2]);
            let dash_is_literal_hyphen =
                !dash.excluded && dash.end - dash.start == 1 && buf[dash.start] == b'-';
            if !a.excluded && !b.excluded && dash_is_literal_hyphen {
                out.push(RawElement { start: a.start, end: b.end, excluded: false });
                i += 3;
                continue;
            }
        }
        let e = &raw[i];
        out.push(RawElement { start: e.start, end: e.end, excluded: e.excluded });
        i += 1;
    }
    out
}

fn report_duplicates(
    ctx: &mut Context<'_>,
    base: u32,
    buf: &[u8],
    elements: &[RawElement],
    interpolations: &[Span],
) {
    let mut seen: Vec<&[u8]> = Vec::new();
    for element in elements {
        let file_span = Span::new(
            base + u32::try_from(element.start).expect("fits u32"),
            base + u32::try_from(element.end).expect("fits u32"),
        );
        if interpolations.iter().any(|i| i.start <= file_span.start && file_span.end <= i.end) {
            continue;
        }
        let text = &buf[element.start..element.end];
        if seen.contains(&text) {
            let fix =
                Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(file_span)] };
            ctx.report_with_fix(&DuplicateRegexpCharacterClassElement::META, file_span, MSG, fix);
        } else {
            seen.push(text);
        }
    }
}
