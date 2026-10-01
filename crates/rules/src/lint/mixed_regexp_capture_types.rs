//! `Lint/MixedRegexpCaptureTypes`, ported from RuboCop's
//! `lib/rubocop/cop/lint/mixed_regexp_capture_types.rb`.
//!
//! # Approach
//!
//! RuboCop parses the regexp's literal source with the `regexp_parser` gem
//! (`node.parsed_tree`, built from the source with any `#{...}`
//! interpolation blanked to spaces first) and walks its expression tree via
//! `each_capture`, which yields every expression that both `capturing?` and
//! either does (`named: true`) or doesn't (`named: false`) `respond_to?
//! (:name)`. The cop fires when both kinds are non-empty. This port skips
//! building any such tree and instead runs a small, purpose-built scanner
//! over the (never-interpolated -- see below) content bytes: it tracks
//! character-class depth (any unescaped `[`/`]`, matching POSIX bracket
//! expressions along with real/nested classes, exactly like the sibling
//! regexp-content rules) and, outside a class, classifies every unescaped
//! `(` it finds as a named capture (`(?<name>` or `(?'name'`, but not the
//! `(?<=`/`(?<!` lookbehind forms), a non-capturing construct (`(?:`, `(?=`,
//! `(?!`, `(?>`, `(?#...)` comments -- consumed up to their own closing
//! `)`, since Oniguruma gives that content no special syntax --, or any
//! other `(?`-prefixed option/modifier group), or else a plain numbered
//! capture. In extended mode (`/x`), an unescaped `#` outside a class also
//! starts a comment running to end of line (matching
//! `redundant_regexp_escape`'s `scan_escapes`), so stray parens inside a
//! free-spacing comment are never misclassified as groups. Because only
//! "does a named/numbered capture exist anywhere" matters (not nesting or
//! matching close parens), the scanner never needs to track paren depth:
//! every `)` is otherwise just an ordinary character.
//!
//! Like the sibling regexp-content rules, this only needs the byte range
//! for a `RegularExpressionNode`'s content; `return if node.interpolation?`
//! is mirrored by never subscribing to `InterpolatedRegularExpressionNode`
//! at all, since an interpolated regexp can never be a `RegularExpressionNode`
//! in Prism's node-kind split (see [`crate::style::redundant_regexp_escape`]).
//!
//! # Blind spots (documented in `META.blind_spots`)
//! RuboCop's `regexp_parser`-based tree fails to build for some malformed
//! patterns (`rescue StandardError` in `assign_properties`), silently
//! yielding no captures at all in that case; this scanner never fails and
//! always finds whatever named/numbered captures its simpler grammar
//! recognizes. No known fixture distinguishes the two: RuboCop's own
//! "cannot be processed by `regexp_parser`" spec example happens to contain
//! only a single (numbered) top-level capture either way, so both
//! approaches agree it is not an offense.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Do not mix named captures and numbered captures in a Regexp literal.";

/// Checks for regexp literals that mix named and numbered captures.
#[derive(Debug, Clone, Default)]
pub struct MixedRegexpCaptureTypes;

impl Rule for MixedRegexpCaptureTypes {
    const META: RuleMeta = RuleMeta {
        name: "Lint/MixedRegexpCaptureTypes",
        department: Department::Lint,
        summary: "Checks for regexp literals that mix named and numbered captures.",
        explanation: "\
Do not mix named captures and numbered captures in a `Regexp` literal
because numbered capture is ignored if they're mixed.
Replace numbered captures with non-capturing groupings or
named captures.

```ruby
# bad
/(?<foo>FOO)(BAR)/

# good
/(?<foo>FOO)(?<bar>BAR)/

# good
/(?<foo>FOO)(?:BAR)/

# good
/(FOO)(BAR)/
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::RegularExpressionNode],
        config: &[],
        blind_spots: "\
RuboCop's `regexp_parser`-based capture scan silently yields no captures at \
all for a pattern it fails to parse (`rescue StandardError`); this port's \
simpler scanner never fails to parse and always finds whatever named/ \
numbered captures its grammar recognizes. No known fixture distinguishes \
the two.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let NodeKind::RegularExpressionNode = node.kind() else { return };
        let Some(n) = node.as_regular_expression_node() else { return };
        let buf = ctx.text(n.content_loc().span());
        let (has_named, has_numbered) = scan_captures(buf, n.is_extended());
        if has_named && has_numbered {
            ctx.report(&Self::META, node.span(), MSG);
        }
    }
}

/// How an unescaped, non-class `(` at some position classifies, and how
/// many bytes of the group's opening syntax to skip past before resuming
/// the normal scan (which then continues over the group's own content --
/// and, for a named group, its name -- as ordinary characters).
enum GroupKind {
    Named,
    Numbered,
    NonCapturing,
}

/// Classifies the group opened by `buf[pos] == '('`. Returns its kind and
/// the number of bytes (starting at `pos`) making up its opening syntax.
fn classify_group(buf: &[u8], pos: usize, len: usize) -> (GroupKind, usize) {
    if pos + 1 >= len || buf[pos + 1] != b'?' {
        return (GroupKind::Numbered, 1);
    }
    if pos + 2 >= len {
        return (GroupKind::NonCapturing, 2);
    }
    match buf[pos + 2] {
        b':' | b'=' | b'!' | b'>' => (GroupKind::NonCapturing, 3),
        b'#' => {
            // `(?#comment)`: Oniguruma gives the content no special syntax
            // (no escaping) -- it simply ends at the first `)`, or at the
            // end of the buffer if there is none.
            let mut i = pos + 3;
            while i < len && buf[i] != b')' {
                i += 1;
            }
            let end = if i < len { i + 1 } else { i };
            (GroupKind::NonCapturing, end - pos)
        }
        b'<' => {
            if pos + 3 < len && matches!(buf[pos + 3], b'=' | b'!') {
                (GroupKind::NonCapturing, 4)
            } else {
                (GroupKind::Named, 3)
            }
        }
        b'\'' => (GroupKind::Named, 3),
        // Inline option/modifier group, e.g. `(?i-mx:...)` or `(?i-mx)`:
        // non-capturing either way. Skip just past `(?`; the option
        // letters, `-`, and terminating `:`/`)` are all ordinary
        // characters to the rest of the scan.
        _ => (GroupKind::NonCapturing, 2),
    }
}

/// Scans `buf` (a regexp literal's content bytes) for top-level named and
/// numbered capture groups, tracking character-class depth (any unescaped
/// `[`/`]`, matching POSIX bracket expressions along with real/nested
/// classes, since only whether depth is non-zero matters here) so that a
/// `(` inside `[...]` is never mistaken for a group, and, outside a class
/// in extended mode (`/x`), skipping `#`-to-end-of-line comments (matching
/// `redundant_regexp_escape`'s `scan_escapes`) so a stray `)`/`(` inside a
/// free-spacing comment is never mistaken for a group either. Returns
/// whether any named capture and whether any numbered capture was found.
fn scan_captures(buf: &[u8], extended: bool) -> (bool, bool) {
    let len = buf.len();
    let mut pos = 0usize;
    let mut depth: u32 = 0;
    let mut has_named = false;
    let mut has_numbered = false;
    while pos < len {
        let c = buf[pos];
        if c == b'\\' {
            pos += 1;
            if pos >= len {
                break;
            }
            pos += utf8_len(buf, pos);
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
        if depth == 0 && extended && c == b'#' {
            while pos < len && buf[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        if depth == 0 && c == b'(' {
            let (kind, advance) = classify_group(buf, pos, len);
            match kind {
                GroupKind::Named => has_named = true,
                GroupKind::Numbered => has_numbered = true,
                GroupKind::NonCapturing => {}
            }
            pos += advance.max(1);
            continue;
        }
        pos += utf8_len(buf, pos);
    }
    (has_named, has_numbered)
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
