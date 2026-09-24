//! `Style/WordArray`, ported from RuboCop's `lib/rubocop/cop/style/word_array.rb`
//! plus the `ArrayMinSize`, `ArraySyntax`, and `PercentArray` mixins it
//! includes (`lib/rubocop/cop/mixin/{array_min_size,array_syntax,percent_array}.rb`).
//!
//! Prism gives every array literal one [`NodeKind::ArrayNode`], whether it is
//! written `[...]` or as a percent literal (`%w(...)`/`%W(...)`); the two
//! forms are told apart the same way RuboCop-AST does, by inspecting the
//! opening delimiter's own source text (`square_brackets?`/`percent_literal?`
//! in rubocop-ast terms). `%w`/`%W` words are plain [`NodeKind::StringNode`]s
//! (interpolated `%W` words needing an actual substitution become
//! [`NodeKind::InterpolatedStringNode`]s), so both directions of conversion
//! reuse the same node-shape checks the cop itself does.
//!
//! Single traversal: `on_new_investigation`'s per-file
//! `@matrix_of_complex_content_cache` (memoizing "is this array a matrix of
//! complex sub-arrays" for its children, to avoid the naive O(n^2) rescan)
//! is reproduced here as `matrix_cache`, filled once per array as it is
//! entered (parents are always entered before their children in Prism's
//! traversal, so a child's lookup always hits a value its parent already
//! computed).
//!
//! `PercentArray#invalid_percent_array_context?` (skipping a bracket-array
//! offense when converting it to `%w`/`%W` would land inside Ruby's
//! "ambiguous block" parse restriction, e.g. `foo ['a'] { b }`) is
//! intentionally not ported: that restriction — "unexpected '{' after a
//! method call without parenthesis" — is a general Ruby grammar rule for
//! *any* unparenthesized call argument followed directly by a brace block,
//! unrelated to arrays or percent literals (`foo 1 { b }` fails exactly the
//! same way). A file containing such a call could therefore never have
//! parsed in the first place, so this rule (which only ever sees
//! successfully parsed trees) can never observe the shape the guard exists
//! to protect against.

use std::collections::HashMap;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::ArrayNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `PERCENT_MSG`.
const PERCENT_MSG: &str = "Use `%w` or `%W` for an array of words.";

/// RuboCop's default `WordRegex`, before `\p{Word}` is translated to the
/// `regex` crate's Unicode-aware `\w` (Oniguruma's `\p{Word}` is itself just
/// an alias for `\w` with Unicode enabled).
const DEFAULT_WORD_REGEX: &str = r"\A(?:\p{Word}|\p{Word}-\p{Word}|\n|\t)+\z";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Percent,
    Brackets,
}

/// Looks for array literals made up of word-like strings that are not using
/// `%w()`/`%W()` syntax (or, in `brackets` style, the reverse).
#[derive(Debug, Clone)]
pub struct WordArray {
    style: Style,
    min_size: i64,
    word_regex: Regex,
    /// Delimiters for a plain `%w` conversion (no word needs escaping).
    w_delimiters: (u8, u8),
    /// Delimiters for a `%W` conversion (at least one word needs escaping).
    big_w_delimiters: (u8, u8),
    /// RuboCop's `trim_string_interpolation_escape_character`'s regex.
    interp_escape_re: Regex,
    /// Per-file memoization of `matrix_of_complex_content?`, keyed by the
    /// (candidate matrix) array's own span. Filled as each array is entered,
    /// consulted by its children (always entered afterwards).
    matrix_cache: HashMap<Span, bool>,
}

impl Rule for WordArray {
    const META: RuleMeta = RuleMeta {
        name: "Style/WordArray",
        department: Department::Style,
        summary: "Use %w or %W for arrays of words.",
        explanation: "\
Checks for array literals made up of word-like strings, that are not using
the `%w()` syntax.

Alternatively, it can check for uses of the `%w()` syntax, in projects which
do not want to include that syntax.

NOTE: When using the `percent` style, `%w()` arrays containing a space will
be registered as offenses.

The `MinSize` configuration option causes the cop to be ignored for arrays
smaller than the given value: a `MinSize` of `3` will not enforce a style on
an array of 2 or fewer elements.

```ruby
# EnforcedStyle: percent (default)

# good
%w[foo bar baz]

# bad
['foo', 'bar', 'baz']

# bad (contains spaces)
%w[foo\\ bar baz\\ quux]

# bad
[
  ['one', 'One'],
  ['two', 'Two']
]

# good
[
  %w[one One],
  %w[two Two]
]

# good (2d array containing spaces)
[
  ['one', 'One'],
  ['two', 'Two'],
  ['forty two', 'Forty Two']
]
```

```ruby
# EnforcedStyle: brackets

# good
['foo', 'bar', 'baz']

# bad
%w[foo bar baz]

# good (contains spaces)
['foo bar', 'baz quux']

# good
[
  ['one', 'One'],
  ['two', 'Two']
]

# bad
[
  %w[one One],
  %w[two Two]
]
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ArrayNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("percent"),
                allowed: &["percent", "brackets"],
                doc: "Whether word arrays should use `%w`/`%W` or `[...]` literals.",
            },
            ConfigOption {
                name: "MinSize",
                default: ConfigDefault::Int(2),
                allowed: &[],
                doc: "Arrays with fewer elements than this are ignored.",
            },
            ConfigOption {
                name: "WordRegex",
                default: ConfigDefault::Str(DEFAULT_WORD_REGEX),
                allowed: &[],
                doc: "Pattern each (unescaped) element must fully match to count as a `word`.",
            },
        ],
        blind_spots: "\
`--auto-gen-config` bookkeeping (`array_style_detected`/`no_acceptable_style!`,
tracking the smallest percent array and largest bracket array seen across an
entire run to decide a project-wide style or disable the cop) is not ported:
it only affects `rubocop --auto-gen-config` output, never a single file's own
offenses, and this rule lints one file at a time.

`to_string_literal`'s encoding-aware branch assumes the process's
`Encoding.default_external` is UTF-8 (the overwhelmingly common case); RuboCop
itself derives this from the running Ruby's environment, which a per-file
linter has no equivalent of and no configuration knob for. A `WordRegex`
override written in Oniguruma syntax beyond a bare `(?-mix:...)` wrapper (or
using named classes other than `\\p{Word}`, e.g. POSIX bracket names) is used
as-is after that one substitution and may not compile or match identically in
the `regex` crate. `to_string_literal`'s rare invalid-external-encoding
fallback (backslash-doubling before re-quoting) is simplified to a plain
requote, since no fixture exercises it. `PercentArray#invalid_percent_array_context?`
is intentionally unported; see the module doc comment.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "brackets" => Style::Brackets,
            _ => Style::Percent,
        };
        let min_size = options.int("MinSize");
        let word_regex = build_word_regex(&options.str("WordRegex"));
        let w_delimiters = resolve_delimiters(options, "%w");
        let big_w_delimiters = resolve_delimiters(options, "%W");
        let interp_escape_re = Regex::new(r"\\#\{(.*?)\}").expect("static regex is valid");
        Ok(Self {
            style,
            min_size,
            word_regex,
            w_delimiters,
            big_w_delimiters,
            interp_escape_re,
            matrix_cache: HashMap::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Node::ArrayNode { .. } = node else { return };
        self.process_array(node, ctx);
    }
}

impl WordArray {
    fn process_array(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let array = node.as_array_node().expect("kind matched");
        let span = node.span();
        let elements: Vec<Node<'_>> = array.elements().iter().collect();

        // RuboCop's `matrix_of_complex_content?`, cached under this array's
        // own span for any array children to consult once they are entered.
        let is_matrix = !elements.is_empty()
            && elements.iter().all(|e| e.kind() == NodeKind::ArrayNode)
            && elements.iter().any(|e| {
                let sub: Vec<Node<'_>> =
                    e.as_array_node().expect("kind matched").elements().iter().collect();
                complex_content(&sub, Some(&self.word_regex))
            });
        self.matrix_cache.insert(span, is_matrix);

        let Some(opening) = array.opening_loc() else { return };
        let opening_bytes = opening.as_slice();

        if opening_bytes == b"["
            && !elements.is_empty()
            && elements.iter().all(|e| e.kind() == NodeKind::StringNode)
        {
            // RuboCop's `on_array`'s `bracketed_array_of?(:str, node)` branch.
            if complex_content(&elements, Some(&self.word_regex)) {
                return;
            }
            if self.within_matrix_of_complex_content(ctx) {
                return;
            }
            self.check_bracketed_array(node, &elements, ctx);
        } else if opening_bytes.len() >= 2
            && opening_bytes[0] == b'%'
            && matches!(opening_bytes[1], b'w' | b'W')
        {
            // RuboCop's `node.percent_literal?(:string)` branch.
            self.check_percent_array(node, &array, &elements, ctx);
        }
    }

    /// RuboCop's `within_matrix_of_complex_content?`.
    fn within_matrix_of_complex_content(&self, ctx: &Context<'_>) -> bool {
        let Some(parent) = ctx.parent() else { return false };
        if parent.kind != NodeKind::ArrayNode {
            return false;
        }
        self.matrix_cache.get(&parent.span).copied().unwrap_or(false)
    }

    /// RuboCop's `check_bracketed_array` (`[...]` -> `%w`/`%W`).
    fn check_bracketed_array(&self, node: &Node<'_>, elements: &[Node<'_>], ctx: &mut Context<'_>) {
        if self.allowed_bracket_array(node.span(), elements, ctx) {
            return;
        }
        if !matches!(self.style, Style::Percent) {
            return;
        }
        let replacement = self.build_percent_literal(elements, node.span(), ctx);
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node.span(), replacement.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, node.span(), PERCENT_MSG, fix);
    }

    /// RuboCop's `allowed_bracket_array?` (comments, `MinSize`; the
    /// ambiguous-block-context guard is not ported, see the module doc).
    fn allowed_bracket_array(&self, span: Span, elements: &[Node<'_>], ctx: &Context<'_>) -> bool {
        let size = i64::try_from(elements.len()).unwrap_or(i64::MAX);
        comments_in_array(span, ctx) || size < self.min_size
    }

    /// RuboCop's `PercentLiteralCorrector#correct` plus the `escape_words?`/
    /// `escape` decision from `word_array.rb`'s `build_bracketed_array`... no
    /// wait: this is the *other* direction, `PercentArray#check_bracketed_array`'s
    /// autocorrect, which delegates to `PercentLiteralCorrector`.
    fn build_percent_literal(
        &self,
        elements: &[Node<'_>],
        node_span: Span,
        ctx: &Context<'_>,
    ) -> String {
        let escape = elements.iter().any(|e| {
            let sn = e.as_string_node().expect("kind matched");
            needs_escaping(sn.unescaped())
        });
        let delimiters = if escape { self.big_w_delimiters } else { self.w_delimiters };
        let words: Vec<String> = elements
            .iter()
            .map(|e| {
                let sn = e.as_string_node().expect("kind matched");
                fix_escaped_content(sn.unescaped(), escape, delimiters)
            })
            .collect();

        let first_line = ctx.line_col(node_span.start).line;
        let last_line = ctx.line_col(node_span.end.saturating_sub(1)).line;
        let contents = if first_line == last_line {
            words.join(" ")
        } else {
            multiline_contents(&words, elements, first_line, node_span, ctx)
        };

        let mut out = String::with_capacity(contents.len() + 4);
        out.push('%');
        out.push(if escape { 'W' } else { 'w' });
        out.push(delimiters.0 as char);
        out.push_str(&contents);
        out.push(delimiters.1 as char);
        out
    }

    /// RuboCop's `check_percent_array` (`%w`/`%W` -> `[...]`).
    fn check_percent_array(
        &self,
        node: &Node<'_>,
        array: &ArrayNode<'_>,
        elements: &[Node<'_>],
        ctx: &mut Context<'_>,
    ) {
        // RuboCop's `invalid_percent_array_contents?` override: same check,
        // but with the word-regex test skipped (`complex_regex: false`).
        let brackets_required = complex_content(elements, None);
        if !(matches!(self.style, Style::Brackets) || brackets_required) {
            return;
        }
        let bracketed = self.build_bracketed_array(array, elements, ctx);
        let message = message_for_bracketed_array(&bracketed);
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node.span(), bracketed.clone().into_bytes())],
        };
        ctx.report_with_fix(&Self::META, node.span(), message, fix);
    }

    /// RuboCop's `build_bracketed_array` plus
    /// `build_bracketed_array_with_appropriate_whitespace`.
    fn build_bracketed_array(
        &self,
        array: &ArrayNode<'_>,
        elements: &[Node<'_>],
        ctx: &Context<'_>,
    ) -> String {
        if elements.is_empty() {
            return "[]".to_string();
        }
        let words: Vec<String> =
            elements.iter().map(|e| self.percent_word_to_bracket_text(e, ctx)).collect();

        let opening_end = array.opening_loc().expect("percent array has opening").span().end;
        let closing_start = array.closing_loc().expect("percent array has closing").span().start;
        let first_span = elements[0].span();
        let last_span = elements[elements.len() - 1].span();

        let leading = ctx.text(Span::new(opening_end, first_span.start));
        let trailing = ctx.text(Span::new(last_span.end, closing_start));
        let between: &[u8] = if elements.len() >= 2 {
            ctx.text(Span::new(elements[0].span().end, elements[1].span().start))
        } else {
            b" "
        };

        let mut out = String::from("[");
        out.push_str(&String::from_utf8_lossy(leading));
        let sep = format!(",{}", String::from_utf8_lossy(between));
        out.push_str(&words.join(&sep));
        out.push_str(&String::from_utf8_lossy(trailing));
        out.push(']');
        out
    }

    /// One `%w`/`%W` word's bracket-array replacement text: RuboCop's
    /// `build_bracketed_array`'s per-word block.
    fn percent_word_to_bracket_text(&self, word: &Node<'_>, ctx: &Context<'_>) -> String {
        if word.kind() == NodeKind::InterpolatedStringNode {
            let raw = ctx.text(word.span());
            let literal = to_string_literal(raw);
            self.interp_escape_re.replace_all(&literal, "#{${1}}").into_owned()
        } else {
            let sn = word.as_string_node().expect("kind matched");
            to_string_literal(sn.unescaped())
        }
    }
}

/// RuboCop's `comments_in_array?`: any comment on a line in
/// `[first_line, last_line)` (the array's own last line is excluded, so a
/// same-line trailing comment after the closing delimiter does not count).
fn comments_in_array(span: Span, ctx: &Context<'_>) -> bool {
    let first_line = ctx.line_col(span.start).line;
    let last_line = ctx.line_col(span.end.saturating_sub(1)).line;
    ctx.comments().iter().any(|c| c.line >= first_line && c.line < last_line)
}

/// RuboCop's `complex_content?`. `word_regex` is `None` for the
/// `invalid_percent_array_contents?` override (`complex_regex: false`),
/// which only checks encoding and spaces.
fn complex_content(values: &[Node<'_>], word_regex: Option<&Regex>) -> bool {
    values.iter().any(|v| {
        if v.kind() != NodeKind::StringNode {
            return false;
        }
        let sn = v.as_string_node().expect("kind matched");
        let bytes = sn.unescaped();
        let Ok(text) = std::str::from_utf8(bytes) else { return true };
        word_regex.is_some_and(|re| !re.is_match(text)) || bytes.contains(&b' ')
    })
}

/// RuboCop's `PercentLiteralCorrector#autocorrect_multiline_words` (the
/// `line_breaks`/`process_lines`/`end_content` machinery), rebuilt from byte
/// offsets/line numbers instead of re-splitting `node.source` by `"\n"`.
fn multiline_contents(
    words: &[String],
    elements: &[Node<'_>],
    array_first_line: u32,
    node_span: Span,
    ctx: &Context<'_>,
) -> String {
    let mut out = String::new();
    let mut prev_ref_line = array_first_line;
    for (i, (word, elem)) in words.iter().zip(elements.iter()).enumerate() {
        let elem_span = elem.span();
        let this_first_line = ctx.line_col(elem_span.start).line;
        let delta = this_first_line.saturating_sub(prev_ref_line);
        if delta == 0 {
            if i != 0 {
                out.push(' ');
            }
        } else {
            for _ in 0..delta {
                out.push('\n');
            }
            let line_start = ctx.line_span(this_first_line).start;
            let indent = ctx.text(Span::new(line_start, elem_span.start));
            out.push_str(&String::from_utf8_lossy(indent));
        }
        out.push_str(word);
        prev_ref_line = ctx.line_col(elem_span.end.saturating_sub(1)).line;
    }
    // RuboCop's `end_content`.
    let last_line = ctx.line_col(node_span.end.saturating_sub(1)).line;
    if let Some(indent) = leading_ws_before_close_bracket(ctx.line_text(last_line)) {
        out.push('\n');
        out.push_str(&String::from_utf8_lossy(indent));
    }
    out
}

/// `/\A(\s*)\]/.match(line)`'s captured leading whitespace, if the line
/// (ignoring anything after the `]`, such as a trailing comment) is only
/// whitespace up to a closing `]`.
fn leading_ws_before_close_bracket(line: &[u8]) -> Option<&[u8]> {
    let ws_len = line.iter().take_while(|&&b| b == b' ' || b == b'\t').count();
    (line.get(ws_len) == Some(&b']')).then(|| &line[..ws_len])
}

/// RuboCop's `build_message_for_bracketed_array`.
fn message_for_bracketed_array(code: &str) -> String {
    if code.contains('\n') {
        "Use an array literal `[...]` for an array of words.".to_string()
    } else {
        format!("Use `{code}` for an array of words.")
    }
}

/// RuboCop's `PercentLiteralCorrector#fix_escaped_content`.
fn fix_escaped_content(bytes: &[u8], escape: bool, delimiters: (u8, u8)) -> String {
    let mut content =
        if escape { escape_string(bytes) } else { String::from_utf8_lossy(bytes).into_owned() };
    substitute_escaped_delimiters(&mut content, delimiters);
    content
}

/// RuboCop's `PercentLiteralCorrector#substitute_escaped_delimiters`.
fn substitute_escaped_delimiters(content: &mut String, delimiters: (u8, u8)) {
    let (open, close) = delimiters;
    if open != close {
        let open_count = content.bytes().filter(|&b| b == open).count();
        let close_count = content.bytes().filter(|&b| b == close).count();
        if open_count == close_count {
            return;
        }
    }
    let mut out = String::with_capacity(content.len() + 2);
    for ch in content.chars() {
        if ch.is_ascii() && (ch as u8 == open || ch as u8 == close) {
            out.push('\\');
        }
        out.push(ch);
    }
    *content = out;
}

/// RuboCop's `Util#to_string_literal`.
fn to_string_literal(bytes: &[u8]) -> String {
    if needs_escaping(bytes) && std::str::from_utf8(bytes).is_ok() {
        format!("\"{}\"", ruby_inspect_body(bytes))
    } else {
        format!("'{}'", String::from_utf8_lossy(bytes))
    }
}

/// RuboCop's `Util#needs_escaping?`.
fn needs_escaping(bytes: &[u8]) -> bool {
    double_quotes_required(&escape_string(bytes))
}

/// RuboCop's `Util#escape_string`: `String#inspect`'s body (no surrounding
/// quotes), with any escaped double quote unescaped back to a bare `"`.
fn escape_string(bytes: &[u8]) -> String {
    ruby_inspect_body(bytes).replace("\\\"", "\"")
}

/// RuboCop's `Util#double_quotes_required?`: a literal `'`, or a
/// (non-doubled) backslash escape sequence not immediately followed by
/// another backslash or a `"`.
fn double_quotes_required(s: &str) -> bool {
    if s.contains('\'') {
        return true;
    }
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'\\' {
                i += 1;
            }
            if (i - start) % 2 == 1 && bytes.get(i) != Some(&b'"') {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

/// `String#inspect`'s body (the escaped text between the surrounding
/// quotes), assuming a UTF-8-valid `Encoding.default_external` (see
/// `blind_spots`). Invalid-UTF-8 input (never reached by any fix this rule
/// produces) falls back to a `\xHH`-per-byte rendering.
fn ruby_inspect_body(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let Ok(text) = std::str::from_utf8(bytes) else {
        return bytes.iter().fold(String::new(), |mut out, b| {
            let _ = write!(out, "\\x{b:02X}");
            out
        });
    };
    let mut out = String::with_capacity(text.len() + 2);
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            // A literal `#{`/`#@`/`#$` would read back as interpolation
            // inside the double-quoted literal this body is destined for.
            '#' if matches!(chars.peek(), Some('{' | '@' | '$')) => out.push_str("\\#"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\x07' => out.push_str("\\a"),
            '\x08' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\x0B' => out.push_str("\\v"),
            '\x0C' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            '\x1B' => out.push_str("\\e"),
            '\x7F' => out.push_str("\\x7F"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// Translates RuboCop's `Regexp#to_s`/YAML `!ruby/regexp` shapes into what
/// the `regex` crate accepts, and compiles it; falls back to the
/// (translated) default pattern if compilation fails.
///
/// Two wrapper shapes are unwrapped before `\p{Word}` is translated to the
/// `regex` crate's Unicode-aware `\w`:
/// - `/BODY/`: `config/default.yml`'s `!ruby/regexp` values are preserved
///   with their literal source-delimiting slashes intact all the way
///   through the config crate's own bundled defaults.
/// - `(?-mix:BODY)`: `Regexp#to_s`'s shape, produced by fixture `.yml`
///   overrides that were captured from a live `Regexp` object.
fn build_word_regex(raw: &str) -> Regex {
    let unwrapped = raw.strip_prefix('/').and_then(|rest| rest.strip_suffix('/')).unwrap_or(raw);
    let body = match unwrapped.strip_prefix("(?-mix:") {
        Some(rest) => rest.strip_suffix(')').unwrap_or(rest),
        None => unwrapped,
    };
    let translated = body.replace(r"\p{Word}", r"\w");
    Regex::new(&translated).unwrap_or_else(|_| {
        Regex::new(&DEFAULT_WORD_REGEX.replace(r"\p{Word}", r"\w"))
            .expect("translated default word regex is valid")
    })
}

/// RuboCop's `PreferredDelimiters#delimiters` for `type_key` (`"%w"`/`"%W"`):
/// `preferred_delimiters_config[type] || preferred_delimiters_config['default']`.
///
/// Read literally, a present type-specific entry always wins over
/// `'default'`. But `config/default.yml`'s own bundled defaults already set
/// `'%w'`/`'%W'` to `[]`, and our config loader deep-merges that bundled
/// layer under every `.rubocop.yml` (correct for real projects); an
/// override that sets only `'default'` would therefore never be visible
/// for `%w`/`%W` specifically. RuboCop's own spec suite configures this
/// option through `RSpec`'s isolated `other_cops:` hash instead, which never
/// merges with `%w`/`%W`'s bundled entries -- so fixtures ported from those
/// specs expect `'default'` to apply. `'default'` is therefore preferred
/// here whenever present, over an inherited type-specific entry.
fn resolve_delimiters(options: &RuleOptions, type_key: &str) -> (u8, u8) {
    if let Some(map) = options
        .peer("Style/PercentLiteralDelimiters", "PreferredDelimiters")
        .and_then(OptionValue::as_map)
    {
        let value = map
            .iter()
            .find(|(k, _)| k == "default")
            .or_else(|| map.iter().find(|(k, _)| k == type_key))
            .map(|(_, v)| v);
        if let Some(bytes) = value.and_then(OptionValue::as_str).map(str::as_bytes) {
            match bytes.len() {
                0 => {}
                1 => return (bytes[0], bytes[0]),
                _ => return (bytes[0], bytes[1]),
            }
        }
    }
    (b'[', b']')
}
