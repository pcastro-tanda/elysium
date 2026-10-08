//! `Lint/MixedCaseRange`, ported from RuboCop's
//! `lib/rubocop/cop/lint/mixed_case_range.rb`.
//!
//! # Approach
//!
//! For `Regexp` literals, RuboCop parses the literal source with the
//! `regexp_parser` gem (`node.parsed_tree`), blanking out any `#{...}`
//! interpolation to spaces of the same width first, so byte offsets still
//! line up with the original source -- the same approach as
//! `style/redundant_regexp_character_class.rs`, whose bracket/escape
//! scanner this file copies privately (`parse_char_class`-equivalent
//! [`scan_char_class`], [`parse_escape`], [`utf8_len`]) to find each
//! top-level `[...]` character class and the two-sided ranges (`X-Y`)
//! directly inside it.
//!
//! A range is flagged only when *both* bounds are a single plain (not
//! escaped) ASCII letter and they fall in different halves of
//! `[Z]`/`[a-z]` -- `regexp_parser`'s `skip_range?` excludes any bound that
//! isn't a plain `:literal` token (an escape, a nested set, ...), and
//! `regexp_range`'s own `return unless range_for(...)` additionally
//! requires *both* bounds to classify as an ASCII letter before an offense
//! (and its replacement) is produced, unlike the plain `Range` literal path
//! below which flags whenever the two bounds classify differently
//! (including one classifying as neither).
//!
//! For the plain `Range` literal path (`'A'..'z'`), upstream's
//! `unsafe_range?` has no such "both known" requirement: it fires whenever
//! `range_for` differs between the two bounds, including when one bound
//! isn't an ASCII letter at all (`'A'..'_'`, `'_'..'a'`) -- confirmed
//! against RuboCop 1.91.0 directly, since the spec suite does not exercise
//! this asymmetry. `Range` objects are never autocorrected (upstream's
//! `NOTE`).
//!
//! # Blind spots (documented in `META.blind_spots`)
//! Ranges inside a *nested* character class (`[a-[A-Z]]`-style set
//! subtraction/intersection) are not checked -- `regexp_parser`'s
//! `each_expression` would process each nested `:set` node independently,
//! but no fixture exercises a range inside one, so the nested bracket is
//! skipped as one opaque token here, matching
//! `style/redundant_regexp_character_class.rs`'s same simplification.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Upstream's `MSG`.
const MSG: &str = "Ranges from upper to lower case ASCII letters may include unintended \
                    characters. Instead of `A-z` (which also includes several symbols) \
                    specify each range individually: `A-Za-z` and individually specify any \
                    symbols.";

/// Checks for mixed-case character ranges since they include likely unintended characters.
#[derive(Debug, Clone)]
pub struct MixedCaseRange;

impl Rule for MixedCaseRange {
    const META: RuleMeta = RuleMeta {
        name: "Lint/MixedCaseRange",
        department: Department::Lint,
        summary: "Checks for mixed-case character ranges since they include likely unintended characters.",
        explanation: "\
Offenses are registered for regexp character classes like `/[A-z]/`
as well as range objects like `('A'..'z')`.

NOTE: `Range` objects cannot be autocorrected.

@safety
  The cop autocorrects regexp character classes
  by replacing one character range with two: `A-z` becomes `A-Za-z`.
  In most cases this is probably what was originally intended
  but it changes the regexp to no longer match symbols it used to include.
  For this reason, this cop's autocorrect is unsafe (it will
  change the behavior of the code).

```ruby
# bad
r = /[A-z]/

# good
r = /[A-Za-z]/
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::RangeNode, NodeKind::RegularExpressionNode, NodeKind::InterpolatedRegularExpressionNode],
        config: &[],
        blind_spots: "\
Ranges inside a nested character class (`[a-[A-Z]]`-style set \
subtraction/intersection) are not checked; the nested bracket is treated as \
one opaque token, matching `Style/RedundantRegexpCharacterClass`'s own \
simplification. No fixture exercises a range inside one.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::RangeNode => check_range_literal(node, ctx),
            NodeKind::RegularExpressionNode => {
                let Some(n) = node.as_regular_expression_node() else { return };
                let span = n.content_loc().span();
                let buf = ctx.text(span).to_vec();
                scan_content(ctx, span.start, &buf);
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
                    scan_content(ctx, base, &buf);
                }
            }
            _ => {}
        }
    }
}

/// `A-Z`/`a-z` classification: `Some(1)` for upper, `Some(0)` for lower,
/// `None` for anything else -- upstream's `range_for`, collapsed to which
/// of the two `RANGES` members (if any) a single ASCII byte falls in.
fn classify(b: u8) -> Option<u8> {
    if b.is_ascii_lowercase() {
        Some(0)
    } else if b.is_ascii_uppercase() {
        Some(1)
    } else {
        None
    }
}

/// `on_irange`/`on_erange`: a plain `Range` literal between two
/// single-character, non-interpolated `String` literals.
fn check_range_literal(node: &Node<'_>, ctx: &mut Context<'_>) {
    let Some(range) = node.as_range_node() else { return };
    let (Some(left), Some(right)) = (range.left(), range.right()) else { return };
    let (Some(l), Some(r)) = (left.as_string_node(), right.as_string_node()) else { return };
    let (lv, rv) = (l.unescaped(), r.unescaped());
    if lv.len() != 1 || rv.len() != 1 {
        return;
    }
    if classify(lv[0]) == classify(rv[0]) {
        return;
    }
    ctx.report(&MixedCaseRange::META, node.span(), MSG);
}

/// Scans `buf` (the regexp's literal content, source bytes with any
/// interpolation blanked to spaces of the same width) for top-level
/// `[...]` character classes, reporting each unsafe range found inside one.
/// `base` is `buf`'s starting byte offset in the file.
fn scan_content(ctx: &mut Context<'_>, base: u32, buf: &[u8]) {
    let len = buf.len();
    let mut pos = 0usize;
    while pos < len {
        let c = buf[pos];
        if c == b'\\' {
            pos += 1 + utf8_len(buf, pos + 1);
            continue;
        }
        if c == b'[' {
            pos = scan_char_class(ctx, base, buf, pos, len);
            continue;
        }
        pos += utf8_len(buf, pos);
    }
}

/// One element (a plain literal character or an escape sequence) parsed at
/// `buf[pos]`: its byte span and whether it was an escape.
fn parse_element(buf: &[u8], pos: usize, len: usize) -> (usize, usize, bool) {
    if buf[pos] == b'\\' {
        let end = parse_escape(buf, pos, len);
        (pos, end, true)
    } else {
        let clen = utf8_len(buf, pos);
        (pos, pos + clen, false)
    }
}

/// Parses a character class starting at `buf[pos] == '['`, reporting any
/// unsafe range directly inside it (not inside a further-nested bracket).
/// Returns the position just past the closing `]`.
fn scan_char_class(ctx: &mut Context<'_>, base: u32, buf: &[u8], pos: usize, len: usize) -> usize {
    let mut p = pos + 1;
    if p < len && buf[p] == b'^' {
        p += 1;
    }
    let mut prev: Option<(usize, usize, bool)> = None;
    let mut first = true;
    while p < len {
        let c = buf[p];
        if c == b']' {
            return p + 1;
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
            prev = None;
            first = false;
            continue;
        }
        if c == b'-' && !first && p + 1 < len && buf[p + 1] != b']' {
            if let Some((s0, s1, s_esc)) = prev {
                let (e0, e1, e_esc) = parse_element(buf, p + 1, len);
                check_range(ctx, base, buf, (s0, s1, s_esc), (e0, e1, e_esc));
                prev = Some((e0, e1, e_esc));
                p = e1;
                first = false;
                continue;
            }
        }
        let (e0, e1, e_esc) = parse_element(buf, p, len);
        prev = Some((e0, e1, e_esc));
        p = e1;
        first = false;
    }
    p
}

/// Upstream's `skip_range?` + `unsafe_range?` + `regexp_range`'s own
/// `return unless range_for(...)` gate, combined: reports only when both
/// bounds are a single plain (non-escaped) ASCII letter in different halves
/// of `A-Z`/`a-z`.
#[allow(clippy::cast_possible_truncation)]
fn check_range(
    ctx: &mut Context<'_>,
    base: u32,
    buf: &[u8],
    start: (usize, usize, bool),
    end: (usize, usize, bool),
) {
    let (s0, s1, s_esc) = start;
    let (e0, e1, e_esc) = end;
    if s_esc || e_esc || s1 - s0 != 1 || e1 - e0 != 1 {
        return;
    }
    let (open, close) = (buf[s0], buf[e0]);
    let (Some(open_class), Some(close_class)) = (classify(open), classify(close)) else {
        return;
    };
    if open_class == close_class {
        return;
    }
    let span = Span::new(base + s0 as u32, base + e1 as u32);
    let replacement = regexp_replacement(open, close);
    let fix =
        Fix { applicability: Applicability::Unsafe, edits: vec![Edit::replace(span, replacement)] };
    ctx.report_with_fix(&MixedCaseRange::META, span, MSG, fix);
}

/// Upstream's `regexp_range`'s merge: `open` and `close` are guaranteed to
/// classify differently, so the other bound's own class determines the
/// complementary boundary letter.
fn regexp_replacement(open: u8, close: u8) -> Vec<u8> {
    let (open_hi, close_lo) = if open.is_ascii_uppercase() { (b'Z', b'a') } else { (b'z', b'A') };
    let mut out = vec![open];
    if open != open_hi {
        out.push(b'-');
        out.push(open_hi);
    }
    if close_lo != close {
        out.push(close_lo);
        out.push(b'-');
    }
    out.push(close);
    out
}

/// Parses one backslash escape starting at `buf[pos] == '\\'`. Returns the
/// position just past it.
fn parse_escape(buf: &[u8], pos: usize, len: usize) -> usize {
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
