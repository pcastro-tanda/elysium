//! `Style/FormatStringToken`, ported from RuboCop's
//! `lib/rubocop/cop/style/format_string_token.rb` plus the format-sequence
//! parser it drives, `lib/rubocop/cop/utils/format_string.rb`.
//!
//! Upstream walks `on_str` (whitequark's unified `str`/`dstr` children) and
//! scans each string's raw source with a single monster regex
//! (`Utils::FormatString::SEQUENCE`) to find every `%`-format token. Prism
//! gives the same shape almost for free: a plain literal is one
//! [`NodeKind::StringNode`], and an interpolated one
//! ([`NodeKind::InterpolatedStringNode`]) splits into `StringNode` literal
//! parts plus `EmbeddedStatementsNode`/`EmbeddedVariableNode` interpolation
//! parts -- finer-grained than whitequark's per-*line* `dstr` children (see
//! the module's trap notes), so a scanned literal segment's raw text never
//! contains a real `#{...}` interpolation to begin with. That lets the
//! sequence scanner below skip `SEQUENCE`'s `INTERPOLATION` alternative
//! inside `WIDTH`/`PRECISION` entirely; the one place a look-alike can still
//! appear is an escaped `\#{name}` next to a template token, which
//! [`scan_curly_name`] guards exactly as upstream's `TEMPLATE_NAME`'s
//! `(?<!\#)` lookbehind does. `Rust`'s `regex` crate has no lookaround or
//! backreference support at all, so the whole grammar is hand-rolled as a
//! backtracking scanner instead, trying the same three type-branch
//! orderings (`WIDTH? PRECISION? NAME?`, `WIDTH? NAME PRECISION?`,
//! `NAME MORE_FLAGS* WIDTH? PRECISION?`) upstream's regex alternation would,
//! before falling back to the template-name branch.
//!
//! `format_string_in_typical_context?`'s `^(send _ {...} %0 ...)` pattern
//! checks the str/dstr node's *direct* parent, which is a `send` node in
//! whitequark (no separate arg-list wrapper) but Prism's
//! [`NodeKind::ArgumentsNode`] in between a `CallNode` and its args. Rather
//! than replaying that ancestor shape from [`Context::ancestors`] (which
//! only carries `(kind, span)`, not the live node), this rule records the
//! handful of node spans that count as "typical format context" --
//! `format`/`sprintf`/`printf`'s first argument, or `%`'s receiver -- into a
//! per-file span set while it still has the live `CallNode`, then just asks
//! whether the literal's (or its enclosing dstr's) own span is in that set.
//! `use_allowed_method?`'s `node.each_ancestor(:send).first` is answered the
//! same way a live parent pointer would: a name stack pushed on entering
//! every non-safe-navigation `CallNode` and popped on leaving it (whitequark
//! never treats a safe-navigation call as `:send`, only `:csend`, so this
//! rule's stack simply never records one, letting the search fall through
//! to the next real enclosing call, matching `each_ancestor(:send)` skipping
//! it).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`, reused for both the configured target style
/// and a detected sequence's own style (`FormatSequence#style`): both are
/// one of the same three values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Annotated,
    Template,
    Unannotated,
}

impl Style {
    fn parse(value: &str) -> Self {
        match value {
            "template" => Self::Template,
            "unannotated" => Self::Unannotated,
            _ => Self::Annotated,
        }
    }

    /// RuboCop's `message_text`.
    const fn message_text(self) -> &'static str {
        match self {
            Self::Annotated => "annotated tokens (like `%<foo>s`)",
            Self::Template => "template tokens (like `%{foo}`)",
            Self::Unannotated => "unannotated tokens (like `%s`)",
        }
    }
}

/// One `%`-token found by [`scan_format_sequences`], restricted to the
/// fields the cop consults (RuboCop's `Utils::FormatString::FormatSequence`,
/// minus `arg_number`/`arity`/`variable_width?`, which this cop never
/// reads). `more_flags` is `FormatSequence#flags`' second half: upstream
/// concatenates `match[:flags]` (before a `<name>`) with `match[:more_flags]`
/// (after one); this scanner keeps them as two spans instead of allocating
/// the concatenation up front.
struct Sequence {
    /// The whole match, `token_range` in the corrector.
    span: Span,
    leading_flags: Span,
    more_flags: Span,
    width: Option<Span>,
    /// Digits after the `.`, *not* including it -- faithfully reproducing
    /// upstream's own `FormatSequence#precision`, whose regex capture group
    /// also excludes the dot. `autocorrect_sequence` concatenates
    /// `flags`/`width`/`precision`/`type` with no separators, so a named
    /// sequence that also carries a precision loses its `.` on conversion
    /// upstream too; see `blind_spots`.
    precision: Option<Span>,
    name: Option<Span>,
    /// `None` for the template branch, which has no type character.
    type_char: Option<u8>,
    style: Style,
}

fn is_flag_byte(b: u8) -> bool {
    matches!(b, b' ' | b'#' | b'0' | b'+' | b'-')
}

/// Ruby's `\w`, approximated: ASCII alnum/underscore, or any non-ASCII byte
/// (UTF-8 lead/continuation), so a Unicode placeholder name still scans as
/// one `\w+` run rather than stopping at its first byte.
fn is_word_byte(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric() || b >= 0x80
}

/// `Utils::FormatString::FLAG` repeated: `[ #0+-]` or a `\d+\$` digit-dollar
/// reference, greedily, any number of times.
fn skip_flags(s: &[u8], mut pos: usize) -> usize {
    loop {
        if pos < s.len() && is_flag_byte(s[pos]) {
            pos += 1;
            continue;
        }
        let start = pos;
        let mut p = pos;
        while p < s.len() && s[p].is_ascii_digit() {
            p += 1;
        }
        if p > start && p < s.len() && s[p] == b'$' {
            pos = p + 1;
            continue;
        }
        return pos;
    }
}

/// `Utils::FormatString::NUMBER`, minus the `#{...}` interpolation
/// alternative (see the module docs): a digit run, or a `*` optionally
/// followed by a whole `\d+\$` digit-dollar arg number. Returns the
/// position just past the match, or `None`.
fn scan_number(s: &[u8], pos: usize) -> Option<usize> {
    let b = *s.get(pos)?;
    if b.is_ascii_digit() {
        let mut p = pos;
        while p < s.len() && s[p].is_ascii_digit() {
            p += 1;
        }
        return Some(p);
    }
    if b == b'*' {
        let start = pos + 1;
        let mut p = start;
        while p < s.len() && s[p].is_ascii_digit() {
            p += 1;
        }
        if p > start && p < s.len() && s[p] == b'$' {
            return Some(p + 1);
        }
        return Some(start);
    }
    None
}

/// `Utils::FormatString::PRECISION`: a `.` followed by an optional
/// [`scan_number`]. Returns the digits' `(start, end)` span (possibly
/// empty) and the position just past the whole match.
fn scan_precision(s: &[u8], pos: usize) -> Option<((usize, usize), usize)> {
    if *s.get(pos)? != b'.' {
        return None;
    }
    let start = pos + 1;
    let end = scan_number(s, start).unwrap_or(start);
    Some(((start, end), end))
}

/// `Utils::FormatString::NAME`: `<(?<name>\w+)>`. Returns the name's
/// `(start, end)` span and the position just past the closing `>`.
fn scan_angle_name(s: &[u8], pos: usize) -> Option<((usize, usize), usize)> {
    if *s.get(pos)? != b'<' {
        return None;
    }
    let start = pos + 1;
    let mut p = start;
    while p < s.len() && is_word_byte(s[p]) {
        p += 1;
    }
    if p == start || *s.get(p)? != b'>' {
        return None;
    }
    Some(((start, p), p + 1))
}

/// `Utils::FormatString::TEMPLATE_NAME`: `(?<!\#)\{(?<name>\w+)\}` -- the
/// negative lookbehind guards an escaped `\#{name}` (a literal `#{...}`-
/// looking run of bytes left in a segment's raw source by `\#`) from being
/// mistaken for a template token.
fn scan_curly_name(s: &[u8], pos: usize) -> Option<((usize, usize), usize)> {
    if *s.get(pos)? != b'{' || (pos > 0 && s[pos - 1] == b'#') {
        return None;
    }
    let start = pos + 1;
    let mut p = start;
    while p < s.len() && is_word_byte(s[p]) {
        p += 1;
    }
    if p == start || *s.get(p)? != b'}' {
        return None;
    }
    Some(((start, p), p + 1))
}

/// `Utils::FormatString::TYPE`: `[bBdiouxXeEfgGaAcps]`.
fn scan_type(s: &[u8], pos: usize) -> Option<u8> {
    let c = *s.get(pos)?;
    matches!(
        c,
        b'b' | b'B'
            | b'd'
            | b'i'
            | b'o'
            | b'u'
            | b'x'
            | b'X'
            | b'e'
            | b'E'
            | b'f'
            | b'g'
            | b'G'
            | b'a'
            | b'A'
            | b'c'
            | b'p'
            | b's'
    )
    .then_some(c)
}

/// Tries to match one `%`-sequence starting at `pos` (a confirmed `%`, not
/// a `%%` escape). Mirrors `SEQUENCE`'s alternation order: the type-branch
/// forms `WIDTH? PRECISION? NAME? TYPE`, `WIDTH? NAME PRECISION? TYPE`,
/// `NAME MORE_FLAGS* WIDTH? PRECISION? TYPE` (each tried in turn, fully
/// backtracking to the flags-end position on failure), then the
/// template-name branch `WIDTH? PRECISION? TEMPLATE_NAME`.
fn scan_sequence(s: &[u8], pos: usize, base: u32) -> Option<Sequence> {
    let span = |a: usize, b: usize| {
        Span::new(
            base + u32::try_from(a).unwrap_or(u32::MAX),
            base + u32::try_from(b).unwrap_or(u32::MAX),
        )
    };
    let flags_start = pos + 1;
    let flags_end = skip_flags(s, flags_start);
    let after_flags = flags_end;
    let leading_flags = span(flags_start, flags_end);
    let empty_more_flags = span(after_flags, after_flags);

    // Attempt A: WIDTH? PRECISION? NAME? TYPE.
    {
        let mut p = after_flags;
        let width = scan_number(s, p).inspect(|&e| p = e);
        let precision = scan_precision(s, p).inspect(|&(_, e)| p = e).map(|(r, _)| r);
        let name = scan_angle_name(s, p).inspect(|&(_, e)| p = e).map(|(r, _)| r);
        if let Some(t) = scan_type(s, p) {
            return Some(Sequence {
                span: span(pos, p + 1),
                leading_flags,
                more_flags: empty_more_flags,
                width: width.map(|e| span(after_flags, e)),
                precision: precision.map(|(a, b)| span(a, b)),
                name: name.map(|(a, b)| span(a, b)),
                type_char: Some(t),
                style: if name.is_some() { Style::Annotated } else { Style::Unannotated },
            });
        }
    }
    // Attempt B: WIDTH? NAME PRECISION? TYPE.
    {
        let mut p = after_flags;
        let width = scan_number(s, p).inspect(|&e| p = e);
        if let Some((name, next)) = scan_angle_name(s, p) {
            p = next;
            let precision = scan_precision(s, p).inspect(|&(_, e)| p = e).map(|(r, _)| r);
            if let Some(t) = scan_type(s, p) {
                return Some(Sequence {
                    span: span(pos, p + 1),
                    leading_flags,
                    more_flags: empty_more_flags,
                    width: width.map(|e| span(after_flags, e)),
                    precision: precision.map(|(a, b)| span(a, b)),
                    name: Some(span(name.0, name.1)),
                    type_char: Some(t),
                    style: Style::Annotated,
                });
            }
        }
    }
    // Attempt C: NAME MORE_FLAGS* WIDTH? PRECISION? TYPE.
    if let Some((name, next)) = scan_angle_name(s, after_flags) {
        let more_start = next;
        let more_end = skip_flags(s, more_start);
        let mut p = more_end;
        let width = scan_number(s, p).inspect(|&e| p = e);
        let precision = scan_precision(s, p).inspect(|&(_, e)| p = e).map(|(r, _)| r);
        if let Some(t) = scan_type(s, p) {
            return Some(Sequence {
                span: span(pos, p + 1),
                leading_flags,
                more_flags: span(more_start, more_end),
                width: width.map(|e| span(more_end, e)),
                precision: precision.map(|(a, b)| span(a, b)),
                name: Some(span(name.0, name.1)),
                type_char: Some(t),
                style: Style::Annotated,
            });
        }
    }
    // Attempt D: WIDTH? PRECISION? TEMPLATE_NAME.
    {
        let mut p = after_flags;
        let width = scan_number(s, p).inspect(|&e| p = e);
        let precision = scan_precision(s, p).inspect(|&(_, e)| p = e).map(|(r, _)| r);
        if let Some((name, next)) = scan_curly_name(s, p) {
            return Some(Sequence {
                span: span(pos, next),
                leading_flags,
                more_flags: empty_more_flags,
                width: width.map(|e| span(after_flags, e)),
                precision: precision.map(|(a, b)| span(a, b)),
                name: Some(span(name.0, name.1)),
                type_char: None,
                style: Style::Template,
            });
        }
    }
    None
}

/// `Utils::FormatString#format_sequences`, minus `%%` escapes (upstream's
/// `token_ranges` skips `detected_sequence.percent?` before ever handing a
/// sequence to the cop, so this scanner drops them just as early instead of
/// modelling them as a `Sequence` at all).
fn scan_format_sequences(content: &[u8], base: u32) -> Vec<Sequence> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < content.len() {
        if content[i] == b'%' {
            if content.get(i + 1) == Some(&b'%') {
                i += 2;
                continue;
            }
            if let Some(seq) = scan_sequence(content, i, base) {
                i = usize::try_from(seq.span.end - base).unwrap_or(content.len());
                out.push(seq);
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Use a consistent style for format string tokens.
#[derive(Debug, Clone)]
pub struct FormatStringToken {
    style: Style,
    max_unannotated_placeholders_allowed: i64,
    conservative: bool,
    allowed_methods: Vec<String>,
    allowed_patterns: Vec<Regex>,
    /// Method name of every currently open non-safe-navigation `CallNode`
    /// ancestor, outermost first; `None` for a safe-navigation one (never
    /// matched, but still a stack slot so `enter`/`leave` stay paired).
    /// Answers `use_allowed_method?`'s `node.each_ancestor(:send).first`.
    call_name_stack: Vec<Option<Vec<u8>>>,
    /// Spans that count as "typical format context" for this file: the
    /// first argument of a `format`/`sprintf`/`printf` call, or the
    /// receiver of a `%` call. Populated while the enclosing `CallNode` is
    /// still live; answers `format_string_in_typical_context?` for a node
    /// (or its enclosing dstr) whose span is a member.
    typical_context_spans: std::collections::HashSet<Span>,
}

impl FormatStringToken {
    fn nearest_send_name(&self) -> Option<&[u8]> {
        self.call_name_stack.iter().rev().find_map(std::option::Option::as_deref)
    }

    fn is_allowed_method(&self, name: &[u8]) -> bool {
        self.allowed_methods.iter().any(|m| m.as_bytes() == name)
            || std::str::from_utf8(name)
                .is_ok_and(|text| self.allowed_patterns.iter().any(|p| p.is_match(text)))
    }

    /// RuboCop's `correctable_sequence?`.
    fn correctable_sequence(&self, detected_type: Option<u8>) -> bool {
        detected_type == Some(b's') || matches!(self.style, Style::Annotated | Style::Unannotated)
    }

    /// RuboCop's `allowed_unannotated?`.
    fn allowed_unannotated(&self, detections: &[Sequence]) -> bool {
        if !detections.iter().all(|d| d.style == Style::Unannotated) {
            return false;
        }
        if i64::try_from(detections.len()).unwrap_or(i64::MAX)
            <= self.max_unannotated_placeholders_allowed
        {
            return true;
        }
        detections.iter().any(|d| !self.correctable_sequence(d.type_char))
    }

    /// RuboCop's `allowed_string?`, given the node's own "is this a
    /// typical-format-context argument" fact (its span's membership in
    /// [`Self::typical_context_spans`]).
    fn allowed_string(&self, detected_style: Style, node_in_typical_context: bool) -> bool {
        (detected_style == Style::Unannotated || self.conservative) && !node_in_typical_context
    }

    fn check_string(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let str_node = node.as_string_node().expect("kind matched");
        let content_span = str_node.content_loc().span();
        let content = ctx.text(content_span);
        if !content.contains(&b'%') {
            return;
        }
        if ctx.ancestors().iter().any(|a| {
            matches!(
                a.kind,
                NodeKind::XStringNode
                    | NodeKind::InterpolatedXStringNode
                    | NodeKind::RegularExpressionNode
                    | NodeKind::InterpolatedRegularExpressionNode
            )
        }) {
            return;
        }
        if self.nearest_send_name().is_some_and(|name| self.is_allowed_method(name)) {
            return;
        }

        let node_in_typical_context = self.typical_context_spans.contains(&node.span());
        let sequences = scan_format_sequences(content, content_span.start);
        let detections: Vec<Sequence> = sequences
            .into_iter()
            .filter(|seq| !self.allowed_string(seq.style, node_in_typical_context))
            .collect();
        if detections.is_empty() || self.allowed_unannotated(&detections) {
            return;
        }

        let format_context = node_in_typical_context
            || ctx.ancestors().iter().any(|a| {
                a.kind == NodeKind::InterpolatedStringNode
                    && self.typical_context_spans.contains(&a.span)
            });
        for seq in &detections {
            self.check_sequence(seq, format_context, ctx);
        }
    }

    /// RuboCop's `check_sequence` + `register_offense`.
    fn check_sequence(&self, seq: &Sequence, format_context: bool, ctx: &mut Context<'_>) {
        if seq.style == self.style {
            return;
        }
        if !self.correctable_sequence(seq.type_char) {
            return;
        }
        let message =
            format!("Prefer {} over {}.", self.style.message_text(), seq.style.message_text());
        if format_context {
            if let Some(fix) = self.build_fix(seq, ctx) {
                ctx.report_with_fix(&Self::META, seq.span, message, fix);
                return;
            }
        }
        ctx.report(&Self::META, seq.span, message);
    }

    /// RuboCop's `autocorrect_sequence`.
    fn build_fix(&self, seq: &Sequence, ctx: &Context<'_>) -> Option<Fix> {
        if self.style == Style::Unannotated {
            return None;
        }
        let name = seq.name?;
        let name_text = ctx.text(name);
        let mut flags_text = ctx.text(seq.leading_flags).to_vec();
        flags_text.extend_from_slice(ctx.text(seq.more_flags));
        let width_text = seq.width.map_or(&[][..], |s| ctx.text(s));
        // `precision` excludes the leading `.`; see the `Sequence` doc.
        let precision_text = seq.precision.map_or(&[][..], |s| ctx.text(s));

        let mut replacement = Vec::new();
        match self.style {
            Style::Annotated => {
                let type_char =
                    if seq.style == Style::Template { b's' } else { seq.type_char.unwrap_or(b's') };
                replacement.extend_from_slice(b"%<");
                replacement.extend_from_slice(name_text);
                replacement.push(b'>');
                replacement.extend_from_slice(&flags_text);
                replacement.extend_from_slice(width_text);
                replacement.extend_from_slice(precision_text);
                replacement.push(type_char);
            }
            Style::Template => {
                replacement.push(b'%');
                replacement.extend_from_slice(&flags_text);
                replacement.extend_from_slice(width_text);
                replacement.extend_from_slice(precision_text);
                replacement.push(b'{');
                replacement.extend_from_slice(name_text);
                replacement.push(b'}');
            }
            Style::Unannotated => unreachable!("returned above"),
        }
        Some(Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(seq.span, replacement)],
        })
    }
}

impl Rule for FormatStringToken {
    const META: RuleMeta = RuleMeta {
        name: "Style/FormatStringToken",
        department: Department::Style,
        summary: "Use a consistent style for format string tokens.",
        explanation: "\
Checks for a consistent style for tokens within a format string.

By default, all strings are evaluated. In some cases, this may be undesirable,
as they could be used as arguments to a method that does not consider them to
be tokens, but rather other identifiers or just part of the string.
`AllowedMethods`/`AllowedPatterns` can mark specific methods as always
allowed, avoiding an offense from the cop. By default, there are no allowed
methods.

Additionally, the cop can be made conservative by configuring it with
`Mode: conservative` (default `aggressive`). In this mode, tokens
(regardless of `EnforcedStyle`) are only considered if used in the format
string argument to the methods `printf`, `sprintf`, `format` and `%`.

NOTE: In `aggressive` mode, offenses are registered for all strings
containing tokens, but autocorrection is only applied when the string
appears in a known formatting context (`format`, `sprintf`, `printf`, or
`%`), to prevent false autocorrections for strings that are not actually
format strings.

NOTE: Tokens in the `unannotated` style (eg. `%s`) are always treated as if
configured with `Conservative: true`, to prevent false positives, because
this format is very similar to encoded URLs or Date/Time formatting
strings.

```ruby
# EnforcedStyle: annotated (default)

# bad
format('%{greeting}', greeting: 'Hello')
format('%s', 'Hello')

# good
format('%<greeting>s', greeting: 'Hello')
```

It is allowed to contain unannotated tokens if the number of them is less
than or equal to `MaxUnannotatedPlaceholdersAllowed` (default `1`).",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StringNode, NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("annotated"),
                allowed: &["annotated", "template", "unannotated"],
                doc: "Which format string token style to enforce.",
            },
            ConfigOption {
                name: "MaxUnannotatedPlaceholdersAllowed",
                default: ConfigDefault::Int(1),
                allowed: &[],
                doc: "Number of unannotated-style tokens allowed in a format string when the \
                      enforced style is not `unannotated`.",
            },
            ConfigOption {
                name: "Mode",
                default: ConfigDefault::Str("aggressive"),
                allowed: &["aggressive", "conservative"],
                doc: "`conservative` only considers strings given to `printf`/`sprintf`/`format`/\
                      `%`; `aggressive` (default) considers every string.",
            },
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method names whose string arguments are always allowed.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method name regex patterns whose string arguments are always allowed.",
            },
        ],
        blind_spots: "\
`AllowedPatterns` entries that fail to compile as a Rust regex are dropped (never match) rather \
than raising a configuration error.

The hand-rolled format-sequence scanner (this rule's replacement for upstream's single \
`Utils::FormatString::SEQUENCE` regex, since Rust's `regex` crate has no lookaround) does not \
model `WIDTH`/`PRECISION`'s `#{...}` interpolation alternative; a real interpolation can never \
reach it (Prism always splits one into its own node before this rule ever sees the literal's raw \
text), so this only matters for a width/precision component that itself contains an escaped \
`\\#{...}`-looking byte run, which is rejected instead of parsed as a dynamic width/precision.

Faithfully reproducing upstream's own `FormatSequence#precision` (whose capture group excludes \
the `.`) and `autocorrect_sequence` (which concatenates `flags`/`width`/`precision`/`type` with \
no separators): converting a named sequence that also carries a precision (e.g. `%<foo>.2f` to \
`template` style) drops the separating `.` from the correction, matching RuboCop's own behavior \
for that combination -- unexercised by this cop's own spec, which never autocorrects a precision \
alongside a name.

RuboCop's `ConfigurableEnforcedStyle` auto-detection (`config_to_allow_offenses`, used only to \
generate a `.rubocop_todo.yml` entry) is not implemented; it does not affect ordinary offense \
reporting.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = Style::parse(options.style("EnforcedStyle")?);
        let conservative = options.style("Mode")? == "conservative";
        let allowed_patterns = options
            .str_list("AllowedPatterns")
            .iter()
            .filter_map(|pattern| Regex::new(pattern).ok())
            .collect();
        Ok(Self {
            style,
            max_unannotated_placeholders_allowed: options.int("MaxUnannotatedPlaceholdersAllowed"),
            conservative,
            allowed_methods: options.str_list("AllowedMethods"),
            allowed_patterns,
            call_name_stack: Vec::new(),
            typical_context_spans: std::collections::HashSet::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.call_name_stack.clear();
        self.typical_context_spans.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                let name = call.name();
                let name = name.as_slice();
                if call.is_safe_navigation() {
                    self.call_name_stack.push(None);
                    return;
                }
                self.call_name_stack.push(Some(name.to_vec()));
                if matches!(name, b"format" | b"sprintf" | b"printf") {
                    if let Some(first_arg) =
                        call.arguments().and_then(|args| args.arguments().iter().next())
                    {
                        self.typical_context_spans.insert(first_arg.span());
                    }
                } else if name == b"%" {
                    if let Some(receiver) = call.receiver() {
                        self.typical_context_spans.insert(receiver.span());
                    }
                }
            }
            NodeKind::StringNode => self.check_string(node, ctx),
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.kind() == NodeKind::CallNode {
            self.call_name_stack.pop();
        }
    }
}
