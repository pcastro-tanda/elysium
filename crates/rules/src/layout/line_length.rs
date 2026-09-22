//! `Layout/LineLength`, ported from RuboCop's `lib/rubocop/cop/layout/line_length.rb`
//! plus its `CheckLineBreakable`, `AllowedPattern`, and `LineLengthHelp` mixins.

use std::collections::{HashMap, HashSet};

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::{BlockNode, BlockParametersNode, CallNode, ParametersNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

/// RuboCop's `MSG`.
fn message(length: i64, max: i64) -> String {
    format!("Line is too long. [{length}/{max}]")
}

/// RuboCop's `AllowHeredoc` cop option: a plain boolean, or a list of
/// heredoc delimiters that are exempt.
#[derive(Debug, Clone)]
enum HeredocAllowance {
    All,
    None,
    Delimiters(Vec<String>),
}

/// One byte position a "\n" (or a string-continuation) can be inserted to
/// shorten an offending line, computed once per file by [`Walker`].
#[derive(Debug, Clone, Copy)]
struct Breakable {
    /// Byte offset to insert the break before.
    pos: u32,
    /// `'` or `"` when the break must be a string continuation
    /// (`"a" \` + `"b"`) rather than a plain newline.
    delimiter: Option<u8>,
}

/// Checks the length of lines in the source code.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct LineLength {
    max: i64,
    allow_heredoc: HeredocAllowance,
    allow_uri: bool,
    allow_qualified_name: bool,
    allow_cop_directives: bool,
    allow_rbs_inline_annotation: bool,
    split_strings: bool,
    autocorrect: bool,
    allowed_patterns: Vec<Regex>,
    uri_regex: Option<Regex>,
    qualified_name_regex: Regex,
    /// RuboCop's `tab_indentation_width`: `nil`/`false` disables the tab
    /// penalty entirely (kept as `None`); otherwise the configured width.
    tab_width: Option<i64>,
    /// Heredoc bodies found in the file: `(first_line, end_line_exclusive,
    /// delimiter)`, matching RuboCop's `extract_heredocs`.
    heredocs: Vec<(u32, u32, String)>,
    /// One breakable insertion point per offending 1-based line, computed
    /// once per file by [`Walker`] and consumed by `file_end`.
    breakable: HashMap<u32, Breakable>,
}

impl Rule for LineLength {
    const META: RuleMeta = RuleMeta {
        name: "Layout/LineLength",
        department: Department::Layout,
        summary: "Checks the length of lines in the source code.",
        explanation: "\
The maximum length is configurable. The tab size is configured in the
`IndentationWidth` of `Layout/IndentationStyle`. A shebang line is ignored
by default.

This cop has some autocorrection capabilities. It can programmatically
shorten certain long lines by inserting line breaks into expressions that
can be safely split across lines -- arrays, hashes, method calls with
argument lists, blocks, and (with `SplitStrings`) string literals.

```ruby
# bad
{foo: \"0000000000\", bar: \"0000000000\", baz: \"0000000000\"}

# good
{foo: \"0000000000\",
bar: \"0000000000\", baz: \"0000000000\"}
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[
            ConfigOption {
                name: "Max",
                default: ConfigDefault::Int(120),
                allowed: &[],
                doc: "Maximum line length in characters.",
            },
            ConfigOption {
                name: "AllowHeredoc",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow long lines inside heredocs (or a list of exempt delimiters).",
            },
            ConfigOption {
                name: "AllowURI",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow long lines that consist mostly of a URI.",
            },
            ConfigOption {
                name: "AllowQualifiedName",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow long lines that consist mostly of a qualified name (`A::B::C`).",
            },
            ConfigOption {
                name: "URISchemes",
                default: ConfigDefault::StrList(&["http", "https"]),
                allowed: &[],
                doc: "URI schemes considered by `AllowURI`.",
            },
            ConfigOption {
                name: "AllowRBSInlineAnnotation",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Allow long lines that are RBS::Inline annotations.",
            },
            ConfigOption {
                name: "AllowCopDirectives",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow long lines that consist mostly of a `# rubocop:` directive.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Lines matching any of these regular expressions are ignored.",
            },
            ConfigOption {
                name: "SplitStrings",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Split long string literals with a continuation when autocorrecting.",
            },
            ConfigOption {
                name: "AutoCorrect",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether this cop may autocorrect offenses.",
            },
        ],
        blind_spots: "\
Autocorrection ports RuboCop's `CheckLineBreakable` mixin without its
ancestor-chain heuristics (`contained_by_breakable_collection_on_same_line?`
and `contained_by_multiline_collection_that_could_be_broken_up?`): a nested
breakable collection/call sharing a line with an already-claimed outer one is
still usually suppressed (the outer claims the line first in traversal
order), but a few deeply nested or already-partially-broken-up cases may
pick a different (or no) breakable point than RuboCop. `AllowURI`'s URI
matcher is a simplified `scheme:\\S+` scan (no RFC 2396 grammar or
`URI.parse` validity check), so malformed URIs that RuboCop would reject are
still treated as exempt. `AllowedPatterns` entries that use Ruby-only regex
syntax (Oniguruma property names, possessive quantifiers) fail to compile
and are silently skipped (the line is then linted normally). Offense
detection itself (the `Max`/`AllowHeredoc`/`AllowURI`/`AllowQualifiedName`/\
`AllowCopDirectives`/`AllowRBSInlineAnnotation`/`AllowedPatterns` line checks)
is a complete port.",
    };

    #[allow(clippy::too_many_lines)]
    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let max = options.int("Max");
        let allow_heredoc = match options.get("AllowHeredoc") {
            Some(OptionValue::Bool(b)) => {
                if *b {
                    HeredocAllowance::All
                } else {
                    HeredocAllowance::None
                }
            }
            Some(other @ (OptionValue::List(_) | OptionValue::Str(_))) => {
                HeredocAllowance::Delimiters(other.to_string_list())
            }
            _ => HeredocAllowance::All,
        };
        let uri_schemes = options.str_list("URISchemes");
        let uri_regex = if uri_schemes.is_empty() {
            None
        } else {
            let alternation =
                uri_schemes.iter().map(|s| regex::escape(s)).collect::<Vec<_>>().join("|");
            Regex::new(&format!(r"(?i)(?:{alternation}):\S+")).ok()
        };
        let qualified_name_regex =
            Regex::new(r"\b(?:[A-Z][A-Za-z0-9_]*::)+[A-Za-z_][A-Za-z0-9_]*\b")
                .expect("static regex is valid");
        let allowed_patterns =
            options.str_list("AllowedPatterns").iter().filter_map(|p| Regex::new(p).ok()).collect();
        let tab_width = tab_indentation_width(options);
        Ok(Self {
            max,
            allow_heredoc,
            allow_uri: options.bool("AllowURI"),
            allow_qualified_name: options.bool("AllowQualifiedName"),
            allow_cop_directives: options.bool("AllowCopDirectives"),
            allow_rbs_inline_annotation: options.bool("AllowRBSInlineAnnotation"),
            split_strings: options.bool("SplitStrings"),
            autocorrect: options.bool("AutoCorrect"),
            allowed_patterns,
            uri_regex,
            qualified_name_regex,
            tab_width,
            heredocs: Vec::new(),
            breakable: HashMap::new(),
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.heredocs.clear();
        self.breakable.clear();

        let comment_lines: HashSet<u32> = ctx.comments().iter().map(|c| c.line).collect();
        let mut walker =
            Walker::new(ctx, self.max, self.tab_width, self.split_strings, comment_lines);
        ruby_ast::walk(&ctx.parsed().root(), &mut walker);

        let mut opaque_spans = walker.opaque_spans;
        opaque_spans.extend(ctx.comments().iter().map(|c| c.span));
        let semicolons = compute_semicolons(ctx, &opaque_spans);

        self.breakable = walker.node_breaks;
        for (line, b) in semicolons {
            self.breakable.insert(line, b);
        }
        for (line, b) in walker.block_breaks {
            self.breakable.insert(line, b);
        }
        self.heredocs = walker.heredocs;
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        // RuboCop inspects `processed_source.lines`, which stops at a
        // `__END__` data section.
        let last_line = match ctx.parsed().data_span() {
            Some(data) => ctx.line_col(data.start).line.saturating_sub(1),
            None => ctx.line_count(),
        };
        for line in 1..=last_line {
            self.check_line(ctx, line);
        }
    }
}

impl LineLength {
    fn check_line(&mut self, ctx: &mut Context<'_>, line: u32) {
        let text = ctx.line_text(line);
        let length = line_length_chars(text, self.tab_width);
        if length <= self.max {
            return;
        }
        if self.allowed_line(text, line) {
            return;
        }
        if self.allow_rbs_inline_annotation && Self::rbs_annotation_on_line(ctx, line) {
            return;
        }
        if self.allow_cop_directives {
            if let Some(directive_start) = directive_start_on_line(ctx, line) {
                self.check_directive_line(ctx, text, line, directive_start);
                return;
            }
        }
        if self.allow_uri || self.allow_qualified_name {
            self.check_line_for_exemptions(ctx, text, line, length);
            return;
        }
        let start = highlight_start(text, self.max, self.tab_width);
        self.report_line(ctx, line, start, length, length);
    }

    fn allowed_line(&self, text: &[u8], line: u32) -> bool {
        self.matches_allowed_pattern(text)
            || (line == 1 && text.starts_with(b"#!"))
            || self.line_in_permitted_heredoc(line)
    }

    fn matches_allowed_pattern(&self, text: &[u8]) -> bool {
        if self.allowed_patterns.is_empty() {
            return false;
        }
        let text = String::from_utf8_lossy(text);
        self.allowed_patterns.iter().any(|re| re.is_match(&text))
    }

    fn line_in_permitted_heredoc(&self, line: u32) -> bool {
        match &self.allow_heredoc {
            HeredocAllowance::None => false,
            HeredocAllowance::All => {
                self.heredocs.iter().any(|(first, end, _)| line >= *first && line < *end)
            }
            HeredocAllowance::Delimiters(list) => self.heredocs.iter().any(|(first, end, d)| {
                line >= *first && line < *end && list.iter().any(|allowed| allowed == d)
            }),
        }
    }

    fn rbs_annotation_on_line(ctx: &Context<'_>, line: u32) -> bool {
        let Some(comment) = ctx.comments().iter().find(|c| c.line == line) else { return false };
        is_rbs_annotation(ctx.text(comment.span))
    }

    fn check_directive_line(
        &mut self,
        ctx: &mut Context<'_>,
        text: &[u8],
        line: u32,
        directive_start: u32,
    ) {
        let line_start = ctx.line_span(line).start;
        let prefix_len = usize::try_from(directive_start.saturating_sub(line_start)).unwrap_or(0);
        let prefix = &text[..prefix_len.min(text.len())];
        let length_without_directive = i64::from(char_count(rstrip(prefix)));
        if length_without_directive <= self.max {
            return;
        }
        self.report_line(ctx, line, self.max, length_without_directive, length_without_directive);
    }

    fn check_line_for_exemptions(
        &mut self,
        ctx: &mut Context<'_>,
        text: &[u8],
        line: u32,
        length: i64,
    ) {
        let uri_range = if self.allow_uri { self.find_uri_range(text, length) } else { None };
        let qn_range = if self.allow_qualified_name {
            self.find_qualified_name_range(text, length)
        } else {
            None
        };
        let allowed = match (uri_range, qn_range) {
            (Some(u), Some(q)) => {
                allowed_position(u, length, self.max) && allowed_position(q, length, self.max)
            }
            (Some(u), None) => allowed_position(u, length, self.max),
            (None, Some(q)) => allowed_position(q, length, self.max),
            (None, None) => false,
        };
        if allowed {
            return;
        }
        let range = uri_range.or(qn_range);
        let start = match range {
            Some((begin, end)) if begin < self.max => end,
            _ => highlight_start(text, self.max, self.tab_width),
        };
        self.report_line(ctx, line, start, length, length);
    }

    /// RuboCop's `find_excessive_range(line, :uri)`.
    fn find_uri_range(&self, text: &[u8], length: i64) -> Option<(i64, i64)> {
        let re = self.uri_regex.as_ref()?;
        let line_str = String::from_utf8_lossy(text);
        let m = re.find_iter(&line_str).filter(|m| is_valid_uri_match(m.as_str())).last()?;
        let begin_col = byte_to_char_col(text, m.start());
        let end_byte = extend_end_position(&line_str, m.end());
        let end_col = byte_to_char_col(text, end_byte);
        let diff = indentation_difference(text, self.tab_width);
        let (begin_col, end_col) = (begin_col + diff, end_col + diff);
        if begin_col < self.max && end_col < self.max {
            return None;
        }
        Some((begin_col, end_col.min(length)))
    }

    /// RuboCop's `find_excessive_range(line, :qualified_name)`.
    fn find_qualified_name_range(&self, text: &[u8], length: i64) -> Option<(i64, i64)> {
        let line_str = String::from_utf8_lossy(text);
        let m = self.qualified_name_regex.find_iter(&line_str).last()?;
        let begin_col = byte_to_char_col(text, m.start());
        let end_byte = extend_end_position(&line_str, m.end());
        let end_col = byte_to_char_col(text, end_byte);
        let diff = indentation_difference(text, self.tab_width);
        let (begin_col, end_col) = (begin_col + diff, end_col + diff);
        if begin_col < self.max && end_col < self.max {
            return None;
        }
        Some((begin_col, end_col.min(length)))
    }

    /// Reports one offense on `line`, columns `[start, end)`, with the fix
    /// from `self.breakable` (if any) attached.
    fn report_line(&mut self, ctx: &mut Context<'_>, line: u32, start: i64, end: i64, length: i64) {
        let start_off = column_to_offset(ctx, line, start);
        let end_off = column_to_offset(ctx, line, end).max(start_off);
        let span = Span::new(start_off, end_off);
        let msg = message(length, self.max);
        match self.autocorrect.then(|| self.breakable.get(&line)).flatten() {
            Some(b) => {
                let insertion = match b.delimiter {
                    Some(d) => vec![d, b' ', b'\\', b'\n', d],
                    None => b"\n".to_vec(),
                };
                let fix = Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::insert(b.pos, insertion)],
                };
                ctx.report_with_fix(&Self::META, span, msg, fix);
            }
            None => ctx.report(&Self::META, span, msg),
        }
    }
}

/// RuboCop's `allowed_position?`.
fn allowed_position(range: (i64, i64), length: i64, max: i64) -> bool {
    range.0 < max && range.1 == length
}

/// RuboCop's `tab_indentation_width`: `Layout/IndentationStyle`'s
/// `IndentationWidth`, else `Layout/IndentationWidth`'s `Width`, else `2`.
/// `false` at any level disables the tab penalty (`None`).
fn tab_indentation_width(options: &RuleOptions) -> Option<i64> {
    if let Some(v) = options.peer("Layout/IndentationStyle", "IndentationWidth") {
        if matches!(v, OptionValue::Bool(false)) {
            return None;
        }
        if let Some(i) = v.as_int() {
            return Some(i);
        }
    }
    if let Some(v) = options.get("IndentationWidth") {
        if matches!(v, OptionValue::Bool(false)) {
            return None;
        }
        if let Some(i) = v.as_int() {
            return Some(i);
        }
    }
    if let Some(v) = options.peer("Layout/IndentationWidth", "Width") {
        if matches!(v, OptionValue::Bool(false)) {
            return None;
        }
        if let Some(i) = v.as_int() {
            return Some(i);
        }
    }
    Some(2)
}

/// RuboCop's `directive_on_source_line?`, returning the directive's start
/// byte offset (used by `check_directive_line`) instead of a bool.
fn directive_start_on_line(ctx: &Context<'_>, line: u32) -> Option<u32> {
    ctx.directives().directives().iter().find(|d| d.line == line).map(|d| d.span.start)
}

/// RuboCop's `rbs_inline_annotation_on_source_line?`: the comment on this
/// line starts with `#:`, `#|`, or `#[...]` (non-empty brackets).
fn is_rbs_annotation(text: &[u8]) -> bool {
    let Some(rest) = text.strip_prefix(b"#") else { return false };
    if rest.starts_with(b":") || rest.starts_with(b"|") {
        return true;
    }
    if let Some(inner) = rest.strip_prefix(b"[") {
        if let Some(close) = inner.iter().position(|&b| b == b']') {
            return close > 0;
        }
    }
    false
}

/// RuboCop's `line_length_help#line_length`: character count plus the tab
/// indentation penalty.
fn line_length_chars(text: &[u8], tab_width: Option<i64>) -> i64 {
    i64::from(char_count(text)) + indentation_difference(text, tab_width)
}

/// RuboCop's `indentation_difference`.
fn indentation_difference(text: &[u8], tab_width: Option<i64>) -> i64 {
    let Some(width) = tab_width else { return 0 };
    let idx = if text.first() == Some(&b'\t') {
        text.iter().position(|&b| b != b'\t').unwrap_or(0)
    } else {
        0
    };
    i64::try_from(idx).unwrap_or(i64::MAX) * (width - 1)
}

/// RuboCop's `highlight_start`.
fn highlight_start(text: &[u8], max: i64, tab_width: Option<i64>) -> i64 {
    (max - indentation_difference(text, tab_width)).max(0)
}

/// Characters (not bytes) in a byte slice, matching RuboCop's columns.
fn char_count(bytes: &[u8]) -> u32 {
    u32::try_from(bytes.iter().filter(|&&b| (b & 0xC0) != 0x80).count()).unwrap_or(u32::MAX)
}

/// Character column of a byte offset within `text`.
fn byte_to_char_col(text: &[u8], byte_offset: usize) -> i64 {
    i64::from(char_count(&text[..byte_offset.min(text.len())]))
}

/// Trims trailing ASCII whitespace, matching Ruby's `String#rstrip` closely
/// enough for the directive-comment prefix this is used on.
fn rstrip(text: &[u8]) -> &[u8] {
    let mut end = text.len();
    while end > 0 && text[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &text[..end]
}

/// Converts a (possibly out-of-line-bounds) 0-based character column on
/// `line` to a byte offset. A column beyond the line's own character count
/// (only possible via the tab-width penalty) extends past the line's bytes
/// using raw byte arithmetic, matching RuboCop's `source_range`, which adds
/// the column directly to the line's start position without clamping.
fn column_to_offset(ctx: &Context<'_>, line: u32, column: i64) -> u32 {
    let span = ctx.line_span(line);
    if column <= 0 {
        return span.start;
    }
    let text = ctx.line_text(line);
    let column = usize::try_from(column).unwrap_or(usize::MAX);
    if let Ok(s) = std::str::from_utf8(text) {
        let mut count = 0usize;
        for (byte_idx, _) in s.char_indices() {
            if count == column {
                return span.start + u32::try_from(byte_idx).unwrap_or(u32::MAX);
            }
            count += 1;
        }
        let overflow = column.saturating_sub(count);
        span.start + u32::try_from(text.len() + overflow).unwrap_or(u32::MAX)
    } else if column <= text.len() {
        span.start + u32::try_from(column).unwrap_or(u32::MAX)
    } else {
        span.start
            + u32::try_from(text.len()).unwrap_or(u32::MAX)
            + u32::try_from(column - text.len()).unwrap_or(u32::MAX)
    }
}

/// Shifts a byte offset by `delta` UTF-8 scalar values (may be negative).
fn shift_chars(source: &[u8], from: u32, delta: i64) -> u32 {
    let mut pos = usize::try_from(from).unwrap_or(0);
    if delta > 0 {
        let mut n = delta;
        while n > 0 && pos < source.len() {
            pos += 1;
            while pos < source.len() && (source[pos] & 0xC0) == 0x80 {
                pos += 1;
            }
            n -= 1;
        }
    } else {
        let mut n = -delta;
        while n > 0 && pos > 0 {
            pos -= 1;
            while pos > 0 && (source[pos] & 0xC0) == 0x80 {
                pos -= 1;
            }
            n -= 1;
        }
    }
    u32::try_from(pos).unwrap_or(u32::MAX)
}

/// RuboCop's `extend_end_position`: extends a URI/qualified-name match's end
/// byte offset (within `line`, a single physical line with no embedded
/// newline) past a trailing `{...}` YARD link and then to the end of the
/// current whitespace-delimited word.
fn extend_end_position(line: &str, end_byte: usize) -> usize {
    let mut end = end_byte;
    if line.contains('{') && line.trim_end_matches(['\r', '\n']).ends_with('}') {
        if let Some(rest) = line.get(end..) {
            if let Some(pos) = rest.rfind('}') {
                end += pos + 1;
            }
        }
    }
    if let Some(rest) = line.get(end..) {
        let word_len: usize =
            rest.chars().take_while(|c| !c.is_whitespace()).map(char::len_utf8).sum();
        end += word_len;
    }
    end
}

/// Scans the raw source for `;` characters outside `opaque` spans (strings,
/// comments, regexes), matching RuboCop's `check_for_breakable_semicolons`.
/// The first valid semicolon on each line wins.
fn compute_semicolons(ctx: &Context<'_>, opaque: &[Span]) -> HashMap<u32, Breakable> {
    let mut result = HashMap::new();
    let bytes = ctx.source().bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b != b';' {
            continue;
        }
        let pos = u32::try_from(i).unwrap_or(u32::MAX);
        if opaque.iter().any(|s| pos >= s.start && pos < s.end) {
            continue;
        }
        let line = ctx.line_col(pos).line;
        if result.contains_key(&line) {
            continue;
        }
        let next_pos = pos + 1;
        if usize::try_from(next_pos).unwrap_or(usize::MAX) >= bytes.len() {
            continue;
        }
        if ctx.line_col(next_pos).line != line {
            continue;
        }
        let next_char = bytes[usize::try_from(next_pos).unwrap_or(0)];
        if next_char == b'\r' || next_char == b'\n' || next_char == b';' {
            continue;
        }
        result.insert(line, Breakable { pos: next_pos, delimiter: None });
    }
    result
}

/// A node considered "heredoc" by RuboCop's `node.heredoc?`: a string-like
/// node whose opening delimiter is `<<...`.
fn is_heredoc_node(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let open = match node {
        Node::StringNode { .. } => node.as_string_node().and_then(|n| n.opening_loc()),
        Node::InterpolatedStringNode { .. } => {
            node.as_interpolated_string_node().and_then(|n| n.opening_loc())
        }
        Node::XStringNode { .. } => {
            Some(node.as_x_string_node().expect("kind matched").opening_loc())
        }
        Node::InterpolatedXStringNode { .. } => {
            Some(node.as_interpolated_x_string_node().expect("kind matched").opening_loc())
        }
        _ => None,
    };
    open.is_some_and(|loc| ctx.text(loc.span()).starts_with(b"<<"))
}

/// Every entry in a `def`'s parameter list, in declaration order.
fn def_parameter_list(params: Option<ParametersNode<'_>>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    let Some(params) = params else { return out };
    out.extend(params.requireds().iter());
    out.extend(params.optionals().iter());
    if let Some(rest) = params.rest() {
        out.push(rest);
    }
    out.extend(params.posts().iter());
    out.extend(params.keywords().iter());
    if let Some(kwrest) = params.keyword_rest() {
        out.push(kwrest);
    }
    if let Some(block) = params.block() {
        out.push(block.as_node());
    }
    out
}

/// RuboCop's `valid_uri?`, approximated: Ruby's stdlib `URI` module
/// registers stricter parsers for a handful of well-known schemes (`ldap`,
/// `mailto`, ...) that reject a bare `scheme:opaque` match lacking the
/// scheme's own required structure (e.g. `ldap:` requires `//`). Every
/// other scheme (including the common `http`/`https` case) accepts any
/// `scheme:opaque-or-hierarchical` text, matching the generic URI grammar.
fn is_valid_uri_match(text: &str) -> bool {
    let Some(colon) = text.find(':') else { return true };
    let scheme = text[..colon].to_ascii_lowercase();
    let rest = &text[colon + 1..];
    match scheme.as_str() {
        "ldap" | "ldaps" | "telnet" | "nntp" | "news" => rest.starts_with("//"),
        _ => true,
    }
}

/// Walks the whole tree once (independent of the engine's own traversal,
/// since this rule has no `META.kinds`), computing every breakable
/// insertion point ([`CheckLineBreakable`]/string-splitting/block-breaking)
/// and collecting heredoc ranges and "opaque" (string/comment) spans for
/// semicolon detection.
/// Cheap, `Copy`able facts about one ancestor on the current walk path
/// (`Node` itself is not `Clone`/`Copy`, so this is what gets pushed/popped
/// instead of the borrowed node).
#[derive(Debug, Clone, Copy)]
struct AncestorInfo {
    kind: NodeKind,
    span: Span,
    /// For an `InterpolatedStringNode` ancestor: its opening quote span.
    interpolated_opening: Option<Span>,
    /// For a `CallNode` ancestor: RuboCop's `receiver_contains_heredoc?`
    /// (the receiver itself, or any of its descendants, is a heredoc).
    call_receiver_contains_heredoc: bool,
    /// RuboCop's `breakable_collection?`: this ancestor is itself an
    /// array/hash/call/def with >= 2 (post-kwarg-flattening) elements,
    /// regardless of whether it ended up claiming a breakable range itself
    /// (e.g. its first element might be a heredoc, suppressing its own
    /// break but not its "I'm breakable" status for nested candidates).
    is_breakable_collection: bool,
    /// RuboCop's `children_could_be_broken_up?`, precomputed for this
    /// ancestor's own elements when it is a breakable collection (used by
    /// `contained_by_multiline_collection_that_could_be_broken_up?`).
    children_could_be_broken_up: bool,
}

struct Walker<'ctx, 'src> {
    ctx: &'ctx Context<'src>,
    max: i64,
    tab_width: Option<i64>,
    split_strings: bool,
    comment_lines: HashSet<u32>,
    ancestors: Vec<AncestorInfo>,
    node_breaks: HashMap<u32, Breakable>,
    block_breaks: HashMap<u32, Breakable>,
    opaque_spans: Vec<Span>,
    heredocs: Vec<(u32, u32, String)>,
}

impl<'ctx, 'src> Walker<'ctx, 'src> {
    fn new(
        ctx: &'ctx Context<'src>,
        max: i64,
        tab_width: Option<i64>,
        split_strings: bool,
        comment_lines: HashSet<u32>,
    ) -> Self {
        Self {
            ctx,
            max,
            tab_width,
            split_strings,
            comment_lines,
            ancestors: Vec::new(),
            node_breaks: HashMap::new(),
            block_breaks: HashMap::new(),
            opaque_spans: Vec::new(),
            heredocs: Vec::new(),
        }
    }

    fn line_of(&self, offset: u32) -> u32 {
        self.ctx.line_col(offset).line
    }

    fn col(&self, offset: u32) -> i64 {
        i64::from(self.ctx.line_col(offset).column)
    }

    fn single_line(&self, span: Span) -> bool {
        self.line_of(span.start)
            == self.line_of(span.end.max(span.start).saturating_sub(1).max(span.start))
    }

    fn ancestor_info(&self, node: &Node<'src>) -> AncestorInfo {
        let interpolated_opening = match node {
            Node::InterpolatedStringNode { .. } => {
                node.as_interpolated_string_node().and_then(|n| n.opening_loc()).map(|l| l.span())
            }
            _ => None,
        };
        let call_receiver_contains_heredoc = match node {
            Node::CallNode { .. } => node.as_call_node().is_some_and(|c| {
                c.receiver().is_some_and(|r| contains_heredoc_descendant(&r, self.ctx))
            }),
            _ => false,
        };
        let (is_breakable_collection, children_could_be_broken_up) = match node {
            Node::ArrayNode { .. } => node.as_array_node().map_or((false, false), |n| {
                let els: Vec<Node<'src>> = n.elements().iter().collect();
                let breakable = els.len() >= 2;
                (breakable, breakable && self.children_could_be_broken_up(&els))
            }),
            Node::HashNode { .. } => node.as_hash_node().map_or((false, false), |n| {
                let els: Vec<Node<'src>> = n.elements().iter().collect();
                let breakable = els.len() >= 2;
                (breakable, breakable && self.children_could_be_broken_up(&els))
            }),
            Node::CallNode { .. } => node.as_call_node().map_or((false, false), |n| {
                let els = call_elements(&n);
                let breakable = els.len() >= 2;
                (breakable, breakable && self.children_could_be_broken_up(&els))
            }),
            Node::DefNode { .. } => node.as_def_node().map_or((false, false), |n| {
                let els = def_parameter_list(n.parameters());
                let breakable = els.len() >= 2;
                (breakable, breakable && self.children_could_be_broken_up(&els))
            }),
            _ => (false, false),
        };
        AncestorInfo {
            kind: node.kind(),
            span: node.span(),
            interpolated_opening,
            call_receiver_contains_heredoc,
            is_breakable_collection,
            children_could_be_broken_up,
        }
    }

    /// RuboCop's `contained_by_breakable_collection_on_same_line?`.
    fn contained_by_breakable_on_same_line(&self, first_line: u32) -> bool {
        for ancestor in self.ancestors.iter().rev() {
            if self.line_of(ancestor.span.start) != first_line {
                return false;
            }
            if ancestor.is_breakable_collection {
                return true;
            }
        }
        false
    }

    /// RuboCop's `contained_by_multiline_collection_that_could_be_broken_up?`:
    /// the *nearest* breakable-collection ancestor (regardless of line)
    /// already has children spread across lines in a way another cop could
    /// clean up further.
    fn contained_by_multiline_breakable(&self) -> bool {
        for ancestor in self.ancestors.iter().rev() {
            if ancestor.is_breakable_collection {
                return ancestor.children_could_be_broken_up;
            }
        }
        false
    }

    /// RuboCop's `children_could_be_broken_up?`.
    fn children_could_be_broken_up(&self, elements: &[Node<'src>]) -> bool {
        let Some(first) = elements.first() else { return false };
        let last = elements.last().expect("non-empty");
        let first_line = self.line_of(first.span().start);
        let last_line = self.last_line_of(last.span());
        if first_line == last_line {
            return false;
        }
        let mut last_seen_line: i64 = -1;
        for el in elements {
            let el_first = i64::from(self.line_of(el.span().start));
            let el_last = i64::from(self.last_line_of(el.span()));
            if last_seen_line >= el_first {
                return true;
            }
            last_seen_line = el_last;
        }
        false
    }

    fn last_line_of(&self, span: Span) -> u32 {
        self.line_of(span.end.saturating_sub(1).max(span.start))
    }

    /// Records a heredoc body's line range (first body line through the
    /// closing delimiter's line, exclusive) and delimiter text. `body_start`
    /// is a byte offset within the body (its first content, or the start of
    /// its first interpolated part) -- not `open.end`, because RuboCop's
    /// stacked heredocs (`foo(<<-A, <<-B)`) put each heredoc's body after
    /// the *previous* heredoc's terminator, not right after the opening
    /// line every time.
    fn record_heredoc(&mut self, open: Span, body_start: u32, close: Span) {
        if !self.ctx.text(open).starts_with(b"<<") {
            return;
        }
        self.opaque_spans.push(Span::new(body_start, close.start));
        let first_line = self.line_of(body_start);
        let end_line = self.line_of(close.start);
        let delimiter = String::from_utf8_lossy(self.ctx.text(close)).trim().to_string();
        self.heredocs.push((first_line, end_line, delimiter));
    }

    /// RuboCop's `check_for_breakable_node`, shared by arrays, hashes,
    /// calls, and defs.
    fn try_breakable_elements(
        &mut self,
        node_span: Span,
        elements: &[Node<'src>],
        drop_first: bool,
        heredoc_shift_applies: bool,
        already_multiline: bool,
    ) {
        let first_line = self.line_of(node_span.start);
        if self.node_breaks.contains_key(&first_line) {
            return;
        }
        if elements.len() < 2 {
            return;
        }
        if already_multiline {
            return;
        }
        if self.contained_by_breakable_on_same_line(first_line) {
            return;
        }
        if self.contained_by_multiline_breakable() {
            return;
        }
        if self.comment_lines.contains(&first_line) {
            return;
        }
        let line_text = self.ctx.line_text(first_line);
        if line_length_chars(line_text, self.tab_width) <= self.max {
            return;
        }
        let effective: &[Node<'src>] =
            if drop_first && elements.len() > 1 { &elements[1..] } else { elements };
        if effective.is_empty() {
            return;
        }
        let mut i = 0usize;
        while i < effective.len() && self.within_column_limit(&effective[i], first_line) {
            i += 1;
        }
        let idx = if heredoc_shift_applies {
            match shift_for_heredoc(effective, i, self.ctx) {
                Some(v) => v,
                None => return,
            }
        } else {
            i
        };
        let chosen = if idx == 0 { &effective[0] } else { &effective[idx - 1] };
        let pos = chosen.span().start;
        self.node_breaks.entry(first_line).or_insert(Breakable { pos, delimiter: None });
    }

    fn within_column_limit(&self, element: &Node<'src>, first_line: u32) -> bool {
        let span = element.span();
        self.col(span.start) <= self.max && self.line_of(span.start) == first_line
    }

    fn chained_to_heredoc(&self, call: &CallNode<'src>) -> bool {
        let mut current = call.receiver();
        while let Some(node) = current {
            if is_heredoc_node(&node, self.ctx) {
                return true;
            }
            current = match &node {
                Node::CallNode { .. } => node.as_call_node().and_then(|c| c.receiver()),
                _ => None,
            };
        }
        false
    }

    fn handle_call(&mut self, node: &Node<'src>) {
        let n = node.as_call_node().expect("kind matched");
        if self.chained_to_heredoc(&n) {
            return;
        }
        let span = n.location().span();
        let already_multiline = !self.single_line(span);
        let elements = call_elements(&n);
        let first_arg_is_heredoc = n
            .arguments()
            .and_then(|a| a.arguments().first())
            .is_some_and(|first| is_heredoc_node(&first, self.ctx));
        let drop_first = n.opening_loc().is_none() && !first_arg_is_heredoc;
        self.try_breakable_elements(span, &elements, drop_first, true, already_multiline);
    }

    fn handle_array(&mut self, node: &Node<'src>) {
        let n = node.as_array_node().expect("kind matched");
        let span = n.location().span();
        let already_multiline = !self.single_line(span);
        let elements: Vec<Node<'src>> = n.elements().iter().collect();
        self.try_breakable_elements(span, &elements, false, true, already_multiline);
    }

    fn handle_hash(&mut self, node: &Node<'src>) {
        let n = node.as_hash_node().expect("kind matched");
        let span = n.location().span();
        let already_multiline = !self.single_line(span);
        let elements: Vec<Node<'src>> = n.elements().iter().collect();
        self.try_breakable_elements(span, &elements, false, false, already_multiline);
    }

    fn handle_def(&mut self, node: &Node<'src>) {
        let n = node.as_def_node().expect("kind matched");
        let span = n.location().span();
        let elements = def_parameter_list(n.parameters());
        let already_multiline = match elements.last() {
            Some(last) => {
                let last_span = last.span();
                self.line_of(span.start)
                    != self.line_of(last_span.end.saturating_sub(1).max(last_span.start))
            }
            None => false,
        };
        self.try_breakable_elements(span, &elements, false, false, already_multiline);
    }

    fn handle_block(&mut self, node: &Node<'src>) {
        let n = node.as_block_node().expect("kind matched");
        let Some(owner) = self.ancestors.last().copied() else { return };
        if !self.single_line(owner.span) {
            return;
        }
        if owner.kind == NodeKind::CallNode && owner.call_receiver_contains_heredoc {
            return;
        }
        let first_line = self.line_of(owner.span.start);
        let pos = breakable_block_pos(&n);
        self.block_breaks.insert(first_line, Breakable { pos, delimiter: None });
    }

    fn handle_lambda(&mut self, node: &Node<'src>) {
        let n = node.as_lambda_node().expect("kind matched");
        let span = n.location().span();
        if !self.single_line(span) {
            return;
        }
        let first_line = self.line_of(span.start);
        let pos = n.opening_loc().span().end;
        self.block_breaks.insert(first_line, Breakable { pos, delimiter: None });
    }

    /// RuboCop's `string_delimiter`: the node's own opening quote, or (for
    /// an unquoted part inside an interpolated string) its parent's.
    fn string_delimiter(&self, own_opening: Option<Span>) -> Option<u8> {
        let span =
            own_opening.or_else(|| self.ancestors.last().and_then(|p| p.interpolated_opening))?;
        match self.ctx.text(span) {
            b"'" => Some(b'\''),
            b"\"" => Some(b'"'),
            _ => None,
        }
    }

    /// RuboCop's `largest_possible_string` + `breakable_string_range`.
    fn breakable_string_range(&self, span: Span) -> Option<u32> {
        let mut max_length = self.max - 3;
        if let Some(parent) = self.ancestors.last() {
            max_length -= self.col(span.start) - self.col(parent.span.start);
        }
        let content = self.ctx.text(span);
        let relevant = take_chars(content, usize::try_from(max_length.max(0)).unwrap_or(0));
        if let Some(char_idx) = last_whitespace_char_index(relevant) {
            return Some(shift_chars(
                self.ctx.source().bytes(),
                span.start,
                i64::try_from(char_idx + 1).unwrap_or(0),
            ));
        }
        if let Some(char_idx) = trailing_escape_char_index(relevant) {
            return Some(shift_chars(
                self.ctx.source().bytes(),
                span.start,
                i64::try_from(char_idx).unwrap_or(0),
            ));
        }
        let end_col = self.col(span.end);
        let adjustment = self.max - end_col - 3;
        let node_len = i64::from(char_count(content));
        if adjustment.abs() > node_len {
            return None;
        }
        Some(shift_chars(self.ctx.source().bytes(), span.end, adjustment))
    }

    fn handle_str(&mut self, node: &Node<'src>) {
        let n = node.as_string_node().expect("kind matched");
        let span = n.location().span();
        if let (Some(open), Some(close)) = (n.opening_loc(), n.closing_loc()) {
            self.record_heredoc(open.span(), n.content_loc().span().start, close.span());
        }
        self.opaque_spans.push(span);

        let first_line = self.line_of(span.start);
        if self.node_breaks.contains_key(&first_line) {
            return;
        }
        if !self.split_strings || !self.single_line(span) {
            return;
        }
        if is_heredoc_node(node, self.ctx) {
            return;
        }
        if matches!(
            self.ancestors.last().map(|p| p.kind),
            Some(
                NodeKind::AssocNode | NodeKind::ArrayNode | NodeKind::OptionalKeywordParameterNode
            )
        ) {
            return;
        }
        let Some(delimiter) = self.string_delimiter(n.opening_loc().map(|l| l.span())) else {
            return;
        };
        if self.col(span.end) < self.max {
            return;
        }
        let Some(pos) = self.breakable_string_range(span) else { return };
        if pos == span.start {
            return;
        }
        self.node_breaks.entry(first_line).or_insert(Breakable { pos, delimiter: Some(delimiter) });
    }

    fn handle_dstr(&mut self, node: &Node<'src>) {
        let n = node.as_interpolated_string_node().expect("kind matched");
        let span = n.location().span();
        if let (Some(open), Some(close)) = (n.opening_loc(), n.closing_loc()) {
            let body_start = n.parts().first().map_or(open.span().end, |p| p.span().start);
            self.record_heredoc(open.span(), body_start, close.span());
        }
        self.opaque_spans.push(span);

        let first_line = self.line_of(span.start);
        if self.node_breaks.contains_key(&first_line) {
            return;
        }
        if !self.split_strings || !self.single_line(span) {
            return;
        }
        if is_heredoc_node(node, self.ctx) {
            return;
        }
        if matches!(
            self.ancestors.last().map(|p| p.kind),
            Some(
                NodeKind::AssocNode | NodeKind::ArrayNode | NodeKind::OptionalKeywordParameterNode
            )
        ) {
            return;
        }
        if n.parts().len() <= 1 {
            return;
        }
        let Some(open) = n.opening_loc() else { return };
        let delimiter = match self.ctx.text(open.span()) {
            b"'" => b'\'',
            b"\"" => b'"',
            _ => return,
        };
        for part in &n.parts() {
            if let Node::EmbeddedStatementsNode { .. } = part {
                let pspan = part.span();
                let start_col = self.col(pspan.start);
                let end_col = self.col(pspan.end);
                if start_col < self.max && end_col >= self.max {
                    self.node_breaks
                        .entry(first_line)
                        .or_insert(Breakable { pos: pspan.start, delimiter: Some(delimiter) });
                    return;
                }
            }
        }
    }

    fn record_opaque_only(&mut self, node: &Node<'src>) {
        self.opaque_spans.push(node.span());
        match node {
            Node::XStringNode { .. } => {
                let n = node.as_x_string_node().expect("kind matched");
                self.record_heredoc(
                    n.opening_loc().span(),
                    n.content_loc().span().start,
                    n.closing_loc().span(),
                );
            }
            Node::InterpolatedXStringNode { .. } => {
                let n = node.as_interpolated_x_string_node().expect("kind matched");
                let body_start =
                    n.parts().first().map_or(n.opening_loc().span().end, |p| p.span().start);
                self.record_heredoc(n.opening_loc().span(), body_start, n.closing_loc().span());
            }
            _ => {}
        }
    }
}

impl<'pr> Visitor<'pr> for Walker<'_, 'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        match node {
            Node::CallNode { .. } => self.handle_call(node),
            Node::ArrayNode { .. } => self.handle_array(node),
            Node::HashNode { .. } => self.handle_hash(node),
            Node::DefNode { .. } => self.handle_def(node),
            Node::BlockNode { .. } => self.handle_block(node),
            Node::LambdaNode { .. } => self.handle_lambda(node),
            Node::StringNode { .. } => self.handle_str(node),
            Node::InterpolatedStringNode { .. } => self.handle_dstr(node),
            Node::XStringNode { .. } | Node::InterpolatedXStringNode { .. } => {
                self.record_opaque_only(node);
            }
            Node::RegularExpressionNode { .. } | Node::InterpolatedRegularExpressionNode { .. } => {
                self.opaque_spans.push(node.span());
            }
            _ => {}
        }
        let info = self.ancestor_info(node);
        self.ancestors.push(info);
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.ancestors.pop();
    }
}

/// RuboCop's `shift_elements_for_heredoc_arg`, applied only to arrays and
/// calls.
fn shift_for_heredoc(elements: &[Node<'_>], index: usize, ctx: &Context<'_>) -> Option<usize> {
    let heredoc_index = elements.iter().position(|e| is_heredoc_node(e, ctx));
    match heredoc_index {
        None => Some(index),
        Some(0) => None,
        Some(h) => Some(if h >= index { index } else { h + 1 }),
    }
}

/// A call's argument list, with a trailing implicit-brace keyword hash
/// flattened into individual associations. Matches RuboCop's
/// `process_args` -- Prism's `KeywordHashNode` is exactly that "hash
/// without braces" case (a real `{...}` hash is a `HashNode`, kept as one
/// element).
fn call_elements<'pr>(n: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut elements: Vec<Node<'pr>> = Vec::new();
    if let Some(args) = n.arguments() {
        elements.extend(args.arguments().iter());
    }
    if matches!(elements.last().map(Node::kind), Some(NodeKind::KeywordHashNode)) {
        let last = elements.pop().expect("checked non-empty above");
        let kw = last.as_keyword_hash_node().expect("kind matched");
        elements.extend(kw.elements().iter());
    }
    elements
}

/// RuboCop's `receiver_contains_heredoc?`: `root` itself, or any of its
/// descendants, is a heredoc string.
fn contains_heredoc_descendant(root: &Node<'_>, ctx: &Context<'_>) -> bool {
    struct Finder<'a, 'b> {
        ctx: &'a Context<'b>,
        found: bool,
    }
    impl<'pr> Visitor<'pr> for Finder<'_, 'pr> {
        fn enter(&mut self, node: &Node<'pr>) {
            if is_heredoc_node(node, self.ctx) {
                self.found = true;
            }
        }
    }
    let mut finder = Finder { ctx, found: false };
    ruby_ast::walk(root, &mut finder);
    finder.found
}

/// RuboCop's `breakable_block_range`, collapsed: in every branch the
/// insertion point is the end of the block's opening delimiter -- the
/// closing `|...|` of its parameter list when it has one, else the opening
/// `{` or `do`.
fn breakable_block_pos(n: &BlockNode<'_>) -> u32 {
    if let Some(params) = n.parameters() {
        if let Node::BlockParametersNode { .. } = params {
            let bp: BlockParametersNode<'_> =
                params.as_block_parameters_node().expect("kind matched");
            if let Some(closing) = bp.closing_loc() {
                return closing.span().end;
            }
        }
    }
    n.opening_loc().span().end
}

/// The first `limit` UTF-8 characters of `bytes` (RuboCop's
/// `node.source[0...max_length]`).
fn take_chars(bytes: &[u8], limit: usize) -> &[u8] {
    if let Ok(s) = std::str::from_utf8(bytes) {
        match s.char_indices().nth(limit) {
            Some((idx, _)) => &bytes[..idx],
            None => bytes,
        }
    } else {
        &bytes[..limit.min(bytes.len())]
    }
}

/// 0-based character index of the last ASCII whitespace char in `bytes`
/// (Ruby's `\s` without `/u`), matching `relevant_substr.rindex(/\s/)`.
fn last_whitespace_char_index(bytes: &[u8]) -> Option<usize> {
    let s = std::str::from_utf8(bytes).ok()?;
    let mut last = None;
    for (idx, c) in s.chars().enumerate() {
        if matches!(c, ' ' | '\t' | '\r' | '\n' | '\x0B' | '\x0C') {
            last = Some(idx);
        }
    }
    last
}

/// 0-based character index where a trailing (possibly partial) escape
/// sequence begins, matching
/// `relevant_substr.rindex(/\\(u[\da-f]{0,4}|x[\da-f]{0,2})?\z/)`.
fn trailing_escape_char_index(bytes: &[u8]) -> Option<usize> {
    let s = std::str::from_utf8(bytes).ok()?;
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    for start in (n.saturating_sub(6)..=n).rev() {
        if start >= n || chars[start] != '\\' {
            continue;
        }
        let rest = &chars[start + 1..];
        let is_hex = |c: &char| c.is_ascii_hexdigit();
        let matches = rest.is_empty()
            || (rest[0] == 'u' && rest.len() <= 5 && rest[1..].iter().all(is_hex))
            || (rest[0] == 'x' && rest.len() <= 3 && rest[1..].iter().all(is_hex));
        if matches {
            return Some(start);
        }
    }
    None
}
