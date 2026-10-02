//! `Lint/RedundantRegexpQuantifiers`, ported from RuboCop's
//! `lib/rubocop/cop/lint/redundant_regexp_quantifiers.rb`.
//!
//! # Approach
//!
//! RuboCop walks `node.parsed_tree` (the `regexp_parser` gem's AST) looking
//! for a non-capturing group (`(?:...)`) that both has exactly one
//! meaningful (non-free-spacing) child and carries its own *greedy*
//! quantifier normalizable to `*`, `?` or `+` (interval forms like `{1,}`
//! count via their normalized quantity). From such a group it walks every
//! descendant in document order, stopping the first time it meets something
//! that is neither itself such a single-child non-capturing group, a
//! character set, nor a leaf (`redundantly_quantifiable?`); every
//! descendant visited along the way that also carries its own mergeable
//! greedy quantifier yields an offense pairing the *outer* group with that
//! descendant, merging their quantifiers (same quantifier wins; otherwise
//! `*`). This port implements the same two-phase walk (collect candidate
//! outer groups across the whole pattern, then chain-descend from each)
//! over a small purpose-built regex-content parser instead of building a
//! `regexp_parser` tree: it tracks `[...]` character-class depth the same
//! way as the sibling regexp-content rules, classifies every unescaped `(`
//! into "non-capturing passive" (`(?:`) vs. everything else (capturing,
//! named, lookaround, atomic, options, comment groups), and recognizes
//! `*`/`+`/`?`/`{m,n}` quantifiers (normalizing `{m,}`/`{,n}`/`{m,n}`) along
//! with their trailing reluctant (`?`) / possessive (`+`) modifier (which
//! disqualifies a quantifier from merging, matching `greedy?`). In extended
//! mode (`/x`) it skips whitespace and `#`-to-end-of-line comments exactly
//! like the sibling regexp-content rules, including when looking past them
//! for a quantifier that follows an atom after intervening free space.
//!
//! Like the sibling regexp-content rules, this only needs the byte range
//! for a `RegularExpressionNode`'s content; `return if node.interpolation?`
//! is mirrored by never subscribing to `InterpolatedRegularExpressionNode`
//! at all, since an interpolated regexp can never be a `RegularExpressionNode`
//! in Prism's node-kind split (see [`crate::style::redundant_regexp_escape`]).
//!
//! Multiple nested redundant pairs in one pattern (e.g.
//! `/(?:(?:a?)+)+/`) can report overlapping fixes in the same round (both
//! pairs drop the same outermost group's quantifier); the engine's
//! overlap-skipping fixer already resolves this the same way RuboCop's own
//! multi-round `expect_correction` does, by applying the non-conflicting
//! fix first and converging over a second round.
//!
//! # Blind spots (documented in `META.blind_spots`)
//! RuboCop's `regexp_parser`-based tree fails to build for some malformed
//! patterns, silently finding no redundant quantifiers at all in that case;
//! this scanner never fails to parse. No known fixture distinguishes the
//! two.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG_REDUNDANT_QUANTIFIER`.
const MSG: &str = "Replace redundant quantifiers ";

/// Checks for redundant quantifiers in Regexps.
#[derive(Debug, Clone, Default)]
pub struct RedundantRegexpQuantifiers;

impl Rule for RedundantRegexpQuantifiers {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RedundantRegexpQuantifiers",
        department: Department::Lint,
        summary: "Checks for redundant quantifiers in Regexps.",
        explanation: "\
It is always allowed when interpolation is used in a regexp literal, \
because it's unknown what kind of string will be expanded as a result:

```ruby
/(?:a*#{interpolation})?/x
```

```ruby
# bad
/(?:x+)+/

# good
/(?:x)+/

# good
/(?:x+)/

# bad
/(?:x+)?/

# good
/(?:x)*/

# good
/(?:x*)/
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::RegularExpressionNode],
        config: &[],
        blind_spots: "\
RuboCop's `regexp_parser`-based tree fails to build for some malformed \
patterns, silently finding no redundant quantifiers at all in that case; \
this scanner never fails to parse. No known fixture distinguishes the two.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let NodeKind::RegularExpressionNode = node.kind() else { return };
        let Some(n) = node.as_regular_expression_node() else { return };
        let content_span = n.content_loc().span();
        let buf = ctx.text(content_span);
        let base = content_span.start;
        let elems = Parser::new(buf, n.is_extended()).parse_root();
        let mut flat = Vec::new();
        flatten(&elems, &mut flat);
        for (expr_idx, child_idx) in find_pairs(&flat) {
            report_pair(ctx, base, &flat[expr_idx], &flat[child_idx]);
        }
    }
}

/// A quantifier (`*`, `+`, `?`, or an interval `{m,n}` form), its own
/// (content-relative) span -- excluding any trailing reluctant/possessive
/// modifier -- and the single-character replacement it normalizes to when
/// greedy (RuboCop's `mergeable_quantifier`), or `None` when it is not
/// greedy or does not normalize to `*`/`?`/`+`.
#[derive(Debug, Clone, Copy)]
struct QuantInfo {
    span: Span,
    greedy: bool,
    symbol: Option<char>,
}

/// One parsed regexp-content element and the quantifier (if any)
/// immediately following it.
struct Elem {
    kind: ElemKind,
    quant: Option<QuantInfo>,
}

enum ElemKind {
    /// A `(...)` group. `passive` is true only for the plain non-capturing
    /// `(?:...)` form (RuboCop's `is?(:passive, :group)`); every other
    /// group syntax (capturing, named, lookaround, atomic, options,
    /// conditional) is `false`.
    Group { passive: bool, children: Vec<Elem> },
    /// A `[...]` character class.
    CharSet,
    /// A top-level `a|b|...` alternation inside the current scope.
    Alt(Vec<Vec<Elem>>),
    /// Anything else: a literal character, an escape sequence, an anchor,
    /// a backreference, or an inline comment/option group -- always a leaf.
    Other,
}

/// Per-element data flattened into document (preorder) order, mirroring
/// what `node.parsed_tree.each_expression` would yield.
struct FlatInfo {
    /// RuboCop's `redundant_group?`: a passive group with exactly one
    /// meaningful child.
    candidate: bool,
    /// RuboCop's `redundantly_quantifiable?`.
    redundantly_quantifiable: bool,
    quant: Option<QuantInfo>,
    /// Number of flattened entries in this element's subtree, including
    /// itself.
    subtree_len: usize,
}

fn mergeable(q: Option<QuantInfo>) -> Option<char> {
    q.and_then(|q| if q.greedy { q.symbol } else { None })
}

fn flatten(elems: &[Elem], out: &mut Vec<FlatInfo>) {
    for e in elems {
        flatten_one(e, out);
    }
}

fn flatten_one(e: &Elem, out: &mut Vec<FlatInfo>) {
    let idx = out.len();
    out.push(FlatInfo {
        candidate: false,
        redundantly_quantifiable: false,
        quant: e.quant,
        subtree_len: 1,
    });
    match &e.kind {
        ElemKind::Group { passive, children } => {
            let candidate = *passive && children.len() == 1;
            let before = out.len();
            flatten(children, out);
            let added = out.len() - before;
            out[idx].candidate = candidate;
            out[idx].redundantly_quantifiable = candidate;
            out[idx].subtree_len = 1 + added;
        }
        ElemKind::CharSet | ElemKind::Other => {
            out[idx].redundantly_quantifiable = true;
        }
        ElemKind::Alt(branches) => {
            let before = out.len();
            for b in branches {
                flatten(b, out);
            }
            let added = out.len() - before;
            out[idx].subtree_len = 1 + added;
        }
    }
}

/// RuboCop's `each_redundantly_quantified_pair`: finds every (outer group,
/// descendant) pair to report, in document order.
fn find_pairs(flat: &[FlatInfo]) -> Vec<(usize, usize)> {
    let mut seen = vec![false; flat.len()];
    let mut pairs = Vec::new();
    for i in 0..flat.len() {
        if seen[i] || !flat[i].candidate || mergeable(flat[i].quant).is_none() {
            continue;
        }
        let subtree_end = i + flat[i].subtree_len;
        for j in (i + 1)..subtree_end {
            seen[j] = true;
            if !flat[j].redundantly_quantifiable {
                break;
            }
            if mergeable(flat[j].quant).is_some() {
                pairs.push((i, j));
            }
        }
    }
    pairs
}

/// RuboCop's `merged_quantifier`: same quantifier wins; otherwise `*`.
fn merged_quantifier(group: char, child: char) -> char {
    if group == child {
        group
    } else {
        '*'
    }
}

#[allow(clippy::cast_possible_truncation)]
fn report_pair(ctx: &mut Context<'_>, base: u32, group: &FlatInfo, child: &FlatInfo) {
    let Some(group_q) = group.quant else { return };
    let Some(child_q) = child.quant else { return };
    let Some(group_sym) = mergeable(Some(group_q)) else { return };
    let Some(child_sym) = mergeable(Some(child_q)) else { return };
    let replacement = merged_quantifier(group_sym, child_sym);
    let group_text = String::from_utf8_lossy(ctx.text(shift(group_q.span, base)));
    let child_text = String::from_utf8_lossy(ctx.text(shift(child_q.span, base)));
    let msg = format!("{MSG}`{child_text}` and `{group_text}` with a single `{replacement}`.");
    let span = Span::new(base + child_q.span.start, base + group_q.span.end);
    let fix = Fix {
        applicability: Applicability::Safe,
        edits: vec![
            Edit::delete(shift(group_q.span, base)),
            Edit::replace(shift(child_q.span, base), replacement.to_string().into_bytes()),
        ],
    };
    ctx.report_with_fix(&RedundantRegexpQuantifiers::META, span, msg, fix);
}

fn shift(span: Span, base: u32) -> Span {
    Span::new(base + span.start, base + span.end)
}

/// Recursive-descent parser over a regexp literal's content bytes.
struct Parser<'a> {
    buf: &'a [u8],
    len: usize,
    pos: usize,
    extended: bool,
}

impl<'a> Parser<'a> {
    fn new(buf: &'a [u8], extended: bool) -> Self {
        Self { buf, len: buf.len(), pos: 0, extended }
    }

    /// Parses the whole pattern (a possible top-level alternation, run to
    /// end of input).
    fn parse_root(&mut self) -> Vec<Elem> {
        self.parse_alt_or_seq()
    }

    fn parse_alt_or_seq(&mut self) -> Vec<Elem> {
        let mut branches = vec![self.parse_sequence()];
        while self.pos < self.len && self.buf[self.pos] == b'|' {
            self.pos += 1;
            branches.push(self.parse_sequence());
        }
        if branches.len() == 1 {
            branches.pop().unwrap_or_default()
        } else {
            vec![Elem { kind: ElemKind::Alt(branches), quant: None }]
        }
    }

    /// Parses atoms until `)`, `|`, or end of input (never consuming the
    /// terminator).
    fn parse_sequence(&mut self) -> Vec<Elem> {
        let mut elems = Vec::new();
        while self.pos < self.len {
            let c = self.buf[self.pos];
            if c == b')' || c == b'|' {
                break;
            }
            if self.extended && c.is_ascii_whitespace() {
                self.pos += 1;
                continue;
            }
            if self.extended && c == b'#' {
                while self.pos < self.len && self.buf[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            let mut elem = match c {
                b'(' => self.parse_group(),
                b'[' => self.parse_charset(),
                b'\\' => self.parse_escape_atom(),
                _ => self.parse_literal_atom(),
            };
            self.attach_quantifier(&mut elem);
            elems.push(elem);
        }
        elems
    }

    /// Looks past extended-mode whitespace/comments (without permanently
    /// consuming them unless a quantifier is actually found) for a
    /// quantifier following the just-parsed atom.
    fn attach_quantifier(&mut self, elem: &mut Elem) {
        let lookahead = if self.extended { self.skip_ext_ws(self.pos) } else { self.pos };
        if let Some((q, new_pos)) = parse_quantifier(self.buf, lookahead, self.len) {
            elem.quant = Some(q);
            self.pos = new_pos;
        }
    }

    fn skip_ext_ws(&self, mut pos: usize) -> usize {
        loop {
            if pos < self.len && self.buf[pos].is_ascii_whitespace() {
                pos += 1;
                continue;
            }
            if pos < self.len && self.buf[pos] == b'#' {
                while pos < self.len && self.buf[pos] != b'\n' {
                    pos += 1;
                }
                continue;
            }
            break;
        }
        pos
    }

    /// Parses a `(...)` group (`self.buf[self.pos] == '('`), classifying it
    /// and recursing into its body when it has one.
    fn parse_group(&mut self) -> Elem {
        self.pos += 1; // consume '('
        if self.pos >= self.len || self.buf[self.pos] != b'?' {
            // Plain capturing group.
            let children = self.parse_alt_or_seq();
            self.consume_close();
            return Elem { kind: ElemKind::Group { passive: false, children }, quant: None };
        }
        let after_q = self.pos + 1;
        if after_q >= self.len {
            self.pos = self.len;
            return Elem { kind: ElemKind::Other, quant: None };
        }
        match self.buf[after_q] {
            b'#' => {
                // `(?#comment)`: ends at the first `)`, no nested syntax.
                let mut i = after_q + 1;
                while i < self.len && self.buf[i] != b')' {
                    i += 1;
                }
                self.pos = if i < self.len { i + 1 } else { i };
                Elem { kind: ElemKind::Other, quant: None }
            }
            b':' => {
                self.pos = after_q + 1;
                let children = self.parse_alt_or_seq();
                self.consume_close();
                Elem { kind: ElemKind::Group { passive: true, children }, quant: None }
            }
            b'=' | b'!' | b'>' => {
                self.pos = after_q + 1;
                let children = self.parse_alt_or_seq();
                self.consume_close();
                Elem { kind: ElemKind::Group { passive: false, children }, quant: None }
            }
            b'<' if after_q + 1 < self.len && matches!(self.buf[after_q + 1], b'=' | b'!') => {
                self.pos = after_q + 2;
                let children = self.parse_alt_or_seq();
                self.consume_close();
                Elem { kind: ElemKind::Group { passive: false, children }, quant: None }
            }
            b'<' => {
                let mut i = after_q + 1;
                while i < self.len && self.buf[i] != b'>' {
                    i += 1;
                }
                self.pos = if i < self.len { i + 1 } else { i };
                let children = self.parse_alt_or_seq();
                self.consume_close();
                Elem { kind: ElemKind::Group { passive: false, children }, quant: None }
            }
            b'\'' => {
                let mut i = after_q + 1;
                while i < self.len && self.buf[i] != b'\'' {
                    i += 1;
                }
                self.pos = if i < self.len { i + 1 } else { i };
                let children = self.parse_alt_or_seq();
                self.consume_close();
                Elem { kind: ElemKind::Group { passive: false, children }, quant: None }
            }
            b'(' => {
                // Conditional group `(?(cond)yes|no)`: treat the remainder
                // generically, like any other group with a body.
                self.pos = after_q;
                let children = self.parse_alt_or_seq();
                self.consume_close();
                Elem { kind: ElemKind::Group { passive: false, children }, quant: None }
            }
            _ => {
                // Inline option/modifier group, e.g. `(?i-mx:...)` or
                // `(?i-mx)`: non-capturing either way.
                let mut i = after_q;
                while i < self.len && (self.buf[i].is_ascii_alphabetic() || self.buf[i] == b'-') {
                    i += 1;
                }
                if i < self.len && self.buf[i] == b':' {
                    self.pos = i + 1;
                    let children = self.parse_alt_or_seq();
                    self.consume_close();
                    Elem { kind: ElemKind::Group { passive: false, children }, quant: None }
                } else {
                    self.pos = if i < self.len && self.buf[i] == b')' { i + 1 } else { i };
                    Elem { kind: ElemKind::Other, quant: None }
                }
            }
        }
    }

    fn consume_close(&mut self) {
        if self.pos < self.len && self.buf[self.pos] == b')' {
            self.pos += 1;
        }
    }

    /// Parses a `[...]` character class (`self.buf[self.pos] == '['`),
    /// tracking bracket depth the same way the sibling regexp-content
    /// rules do (any unescaped `[`/`]`, matching POSIX bracket expressions
    /// along with real/nested classes, since only whether depth is
    /// non-zero matters here).
    fn parse_charset(&mut self) -> Elem {
        self.pos += 1; // consume '['
        let mut depth: u32 = 0;
        while self.pos < self.len {
            match self.buf[self.pos] {
                b'\\' => {
                    self.pos += 1;
                    if self.pos < self.len {
                        self.pos += utf8_len(self.buf, self.pos);
                    }
                }
                b'[' => {
                    depth += 1;
                    self.pos += 1;
                }
                b']' => {
                    self.pos += 1;
                    if depth > 0 {
                        depth -= 1;
                    } else {
                        break;
                    }
                }
                _ => self.pos += utf8_len(self.buf, self.pos),
            }
        }
        Elem { kind: ElemKind::CharSet, quant: None }
    }

    /// Parses a backslash escape (`self.buf[self.pos] == '\\'`), consuming
    /// through the matching closer for the bracketed/braced forms so later
    /// parsing never misreads their contents as regex syntax.
    fn parse_escape_atom(&mut self) -> Elem {
        self.pos += 1; // consume '\\'
        if self.pos >= self.len {
            return Elem { kind: ElemKind::Other, quant: None };
        }
        let c = self.buf[self.pos];
        let bracketed =
            matches!(c, b'k' | b'g') && matches!(self.buf.get(self.pos + 1), Some(b'<' | b'\''));
        let braced =
            matches!(c, b'p' | b'P' | b'x' | b'u') && self.buf.get(self.pos + 1) == Some(&b'{');
        if bracketed {
            let close = if self.buf[self.pos + 1] == b'<' { b'>' } else { b'\'' };
            self.pos += 2;
            while self.pos < self.len && self.buf[self.pos] != close {
                self.pos += 1;
            }
            if self.pos < self.len {
                self.pos += 1;
            }
        } else if braced {
            self.pos += 2;
            while self.pos < self.len && self.buf[self.pos] != b'}' {
                self.pos += 1;
            }
            if self.pos < self.len {
                self.pos += 1;
            }
        } else {
            self.pos += utf8_len(self.buf, self.pos);
        }
        Elem { kind: ElemKind::Other, quant: None }
    }

    fn parse_literal_atom(&mut self) -> Elem {
        self.pos += utf8_len(self.buf, self.pos);
        Elem { kind: ElemKind::Other, quant: None }
    }
}

/// Parses a quantifier (`*`, `+`, `?`, or `{m,n}`/`{m,}`/`{,n}`/`{m}`)
/// starting at `buf[pos]`, including any trailing reluctant (`?`) or
/// possessive (`+`) modifier (which disqualifies it from merging, matching
/// `greedy?`). Returns the quantifier info (whose `span` covers only the
/// base token, not the modifier) and the position just past the whole
/// token (modifier included).
#[allow(clippy::cast_possible_truncation)]
fn parse_quantifier(buf: &[u8], pos: usize, len: usize) -> Option<(QuantInfo, usize)> {
    if pos >= len {
        return None;
    }
    let (quantity, base_end) = match buf[pos] {
        b'*' => ((0i32, -1i32), pos + 1),
        b'+' => ((1, -1), pos + 1),
        b'?' => ((0, 1), pos + 1),
        b'{' => {
            let mut i = pos + 1;
            let min_start = i;
            while i < len && buf[i].is_ascii_digit() {
                i += 1;
            }
            let has_min = i > min_start;
            let min_val: i32 = if has_min {
                std::str::from_utf8(&buf[min_start..i]).ok()?.parse().ok()?
            } else {
                0
            };
            let max_val: i32 = if i < len && buf[i] == b',' {
                i += 1;
                let max_start = i;
                while i < len && buf[i].is_ascii_digit() {
                    i += 1;
                }
                if i > max_start {
                    std::str::from_utf8(&buf[max_start..i]).ok()?.parse().ok()?
                } else {
                    -1
                }
            } else {
                if !has_min {
                    return None;
                }
                min_val
            };
            if i >= len || buf[i] != b'}' {
                return None;
            }
            i += 1;
            ((min_val, max_val), i)
        }
        _ => return None,
    };
    let mut end = base_end;
    let mut greedy = true;
    if end < len && matches!(buf[end], b'?' | b'+') {
        greedy = false;
        end += 1;
    }
    let symbol = if greedy {
        match quantity {
            (0, -1) => Some('*'),
            (0, 1) => Some('?'),
            (1, -1) => Some('+'),
            _ => None,
        }
    } else {
        None
    };
    Some((QuantInfo { span: Span::new(pos as u32, base_end as u32), greedy, symbol }, end))
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
