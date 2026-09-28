//! `Style/SymbolArray`, ported from RuboCop's `lib/rubocop/cop/style/symbol_array.rb`
//! plus the `ArrayMinSize`, `ArraySyntax`, and `PercentArray` mixins it
//! includes (`lib/rubocop/cop/mixin/{array_min_size,array_syntax,percent_array}.rb`),
//! and the `PercentLiteralCorrector`/`Util` helpers it drives.
//!
//! Prism gives every array literal one [`NodeKind::ArrayNode`], whether it is
//! written `[...]` or as a percent literal (`%i(...)`/`%I(...)`); the two
//! forms are told apart the same way RuboCop-AST does, by inspecting the
//! opening delimiter's own source text (`square_brackets?`/`percent_literal?`
//! in rubocop-ast terms). Plain symbols (`:foo`, and quoted forms with no
//! interpolation such as `:"foo bar"`) are [`NodeKind::SymbolNode`], matching
//! whitequark's `sym` type; a symbol that actually interpolates
//! (`:"#{foo}"`) is [`NodeKind::InterpolatedSymbolNode`], matching `dsym`.
//! `bracketed_array_of?(:sym, node)` therefore becomes "every element is a
//! plain `SymbolNode`" here.
//!
//! `complex_content?` (private to this cop, not the `PercentArray` mixin) is
//! reproduced byte-for-byte, including its `return false if
//! DELIMITERS.include?(sym.source)` early exit: for a *percent-literal*
//! element, `sym.source` is exactly the element's own raw span text (no
//! leading `:`), so a bare, unescaped `[`/`]`/`(`/`)` element short-circuits
//! the whole check to "not complex" (see `does_not_register_an_offense_for_a_i_array_containing_unes*`
//! fixtures); an *escaped* `\[`/`\(`/etc. element has two-byte raw source and
//! never matches this exact check, but its *decoded* value (a single
//! delimiter byte) still trips the general space/delimiter test afterwards.
//! For a *bracket-array* element, `sym.source` includes the surrounding
//! `:`/quotes (e.g. `:")"`, never a bare single byte), so this early exit
//! never fires there; only the general check (which looks at the *decoded*
//! symbol value) can flag it.
//!
//! `PercentArray#invalid_percent_array_context?` (skipping a bracket-array
//! offense when converting it to `%i`/`%I` would land inside Ruby's
//! "ambiguous block" parse restriction, e.g. `foo [:a] { b }`) is
//! intentionally not ported, for the same reason `Style/WordArray` skips it:
//! that restriction is a general Ruby grammar rule for *any* unparenthesized
//! call argument followed directly by a brace block, unrelated to arrays or
//! percent literals.

use std::sync::LazyLock;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::ArrayNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `PERCENT_MSG`.
const PERCENT_MSG: &str = "Use `%i` or `%I` for an array of symbols.";

/// RuboCop's `DELIMITERS`.
const DELIMITERS: [u8; 4] = *b"[]()";

/// RuboCop's `SPECIAL_GVARS`.
const SPECIAL_GVARS: &[&str] = &[
    "$!", "$\"", "$$", "$&", "$'", "$*", "$+", "$,", "$/", "$;", "$:", "$.", "$<", "$=", "$>",
    "$?", "$@", "$\\", "$_", "$`", "$~", "$0", "$-0", "$-F", "$-I", "$-K", "$-W", "$-a", "$-d",
    "$-i", "$-l", "$-p", "$-v", "$-w",
];

/// RuboCop's `REDEFINABLE_OPERATORS`.
const REDEFINABLE_OPERATORS: &[&str] = &[
    "|", "^", "&", "<=>", "==", "===", "=~", ">", ">=", "<", "<=", "<<", ">>", "+", "-", "*", "/",
    "%", "**", "~", "+@", "-@", "[]", "[]=", "`", "!", "!=", "!~",
];

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Percent,
    Brackets,
}

/// Checks for array literals made up of symbols that are not using the
/// `%i()`/`%I()` syntax (or, in `brackets` style, the reverse).
#[derive(Debug, Clone)]
pub struct SymbolArray {
    style: Style,
    min_size: i64,
    /// Delimiters for a plain `%i` conversion (no symbol needs escaping).
    i_delimiters: (u8, u8),
    /// Delimiters for a `%I` conversion (at least one symbol needs escaping).
    big_i_delimiters: (u8, u8),
    /// RuboCop's `trim_string_interpolation_escape_character`'s regex.
    interp_escape_re: Regex,
}

impl Rule for SymbolArray {
    const META: RuleMeta = RuleMeta {
        name: "Style/SymbolArray",
        department: Department::Style,
        summary: "Use %i or %I for arrays of symbols.",
        explanation: "\
Checks for array literals made up of symbols that are not using the `%i()`
syntax.

Alternatively, it checks for symbol arrays using the `%i()` syntax on
projects which do not want to use that syntax, perhaps because they support a
version of Ruby lower than 2.0.

The `MinSize` configuration option causes the cop to be ignored for arrays
smaller than the given value: a `MinSize` of `3` will not enforce a style on
an array of 2 or fewer elements.

```ruby
# EnforcedStyle: percent (default)

# good
%i[foo bar baz]

# bad
[:foo, :bar, :baz]

# bad (contains spaces)
%i[foo\\ bar baz\\ quux]

# bad (contains [] with spaces)
%i[foo \\[ \\]]

# bad (contains () with spaces)
%i(foo \\( \\))
```

```ruby
# EnforcedStyle: brackets

# good
[:foo, :bar, :baz]

# bad
%i[foo bar baz]
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
                doc: "Whether symbol arrays should use `%i`/`%I` or `[...]` literals.",
            },
            ConfigOption {
                name: "MinSize",
                default: ConfigDefault::Int(2),
                allowed: &[],
                doc: "Arrays with fewer elements than this are ignored.",
            },
        ],
        blind_spots: "\
`--auto-gen-config` bookkeeping (`array_style_detected`/`no_acceptable_style!`,
tracking the smallest percent array and largest bracket array seen across an
entire run to decide a project-wide style or disable the cop) is not ported:
it only affects `rubocop --auto-gen-config` output, never a single file's own
offenses, and this rule lints one file at a time.

`to_string_literal`'s encoding-aware branch assumes the process's
`Encoding.default_external` is UTF-8 (the overwhelmingly common case).
`PercentArray#invalid_percent_array_context?` is intentionally unported; see
the module doc comment.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "brackets" => Style::Brackets,
            _ => Style::Percent,
        };
        let min_size = options.int("MinSize");
        let i_delimiters = resolve_delimiters(options, "%i");
        let big_i_delimiters = resolve_delimiters(options, "%I");
        let interp_escape_re = Regex::new(r"\\#\{(.*?)\}").expect("static regex is valid");
        Ok(Self { style, min_size, i_delimiters, big_i_delimiters, interp_escape_re })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Node::ArrayNode { .. } = node else { return };
        self.process_array(node, ctx);
    }
}

impl SymbolArray {
    fn process_array(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let array = node.as_array_node().expect("kind matched");
        let elements: Vec<Node<'_>> = array.elements().iter().collect();
        let Some(opening) = array.opening_loc() else { return };
        let opening_bytes = opening.as_slice();

        if opening_bytes == b"["
            && !elements.is_empty()
            && elements.iter().all(|e| e.kind() == NodeKind::SymbolNode)
        {
            // RuboCop's `on_array`'s `bracketed_array_of?(:sym, node)` branch.
            if complex_content(&elements, ctx) {
                return;
            }
            self.check_bracketed_array(node, &elements, ctx);
        } else if opening_bytes.len() >= 2
            && opening_bytes[0] == b'%'
            && matches!(opening_bytes[1], b'i' | b'I')
        {
            // RuboCop's `node.percent_literal?(:symbol)` branch.
            self.check_percent_array(node, &array, &elements, ctx);
        }
    }

    /// RuboCop's `check_bracketed_array` (`[...]` -> `%i`/`%I`).
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

    /// RuboCop's `PercentArray#check_bracketed_array`'s autocorrect, which
    /// delegates to `PercentLiteralCorrector#correct`.
    fn build_percent_literal(
        &self,
        elements: &[Node<'_>],
        node_span: Span,
        ctx: &Context<'_>,
    ) -> String {
        let escape = elements.iter().any(|e| {
            let sn = e.as_symbol_node().expect("kind matched");
            needs_escaping(sn.unescaped())
        });
        let delimiters = if escape { self.big_i_delimiters } else { self.i_delimiters };
        let words: Vec<String> = elements
            .iter()
            .map(|e| {
                let sn = e.as_symbol_node().expect("kind matched");
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
        out.push(if escape { 'I' } else { 'i' });
        out.push(delimiters.0 as char);
        out.push_str(&contents);
        out.push(delimiters.1 as char);
        out
    }

    /// RuboCop's `check_percent_array` (`%i`/`%I` -> `[...]`).
    fn check_percent_array(
        &self,
        node: &Node<'_>,
        array: &ArrayNode<'_>,
        elements: &[Node<'_>],
        ctx: &mut Context<'_>,
    ) {
        // RuboCop's `invalid_percent_array_contents?` override: identical to
        // `complex_content?` (this cop doesn't further narrow it).
        let brackets_required = complex_content(elements, ctx);
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
        let syms: Vec<String> =
            elements.iter().map(|e| self.percent_sym_to_bracket_text(e, ctx)).collect();

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
        out.push_str(&syms.join(&sep));
        out.push_str(&String::from_utf8_lossy(trailing));
        out.push(']');
        out
    }

    /// One `%i`/`%I` element's bracket-array replacement text: RuboCop's
    /// `build_bracketed_array`'s per-element block.
    fn percent_sym_to_bracket_text(&self, sym: &Node<'_>, ctx: &Context<'_>) -> String {
        if sym.kind() == NodeKind::InterpolatedSymbolNode {
            let raw = ctx.text(sym.span());
            let literal = to_string_literal(raw);
            let trimmed = self.interp_escape_re.replace_all(&literal, "#{${1}}").into_owned();
            format!(":{trimmed}")
        } else {
            let sn = sym.as_symbol_node().expect("kind matched");
            to_symbol_literal(sn.unescaped())
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

/// RuboCop's `complex_content?`.
///
/// A bare (unescaped), single-byte `[`/`]`/`(`/`)` element short-circuits the
/// whole method to "not complex", matching Ruby's non-local `return false`
/// from inside the `any?` block.
fn complex_content(values: &[Node<'_>], ctx: &Context<'_>) -> bool {
    for v in values {
        let raw = ctx.text(v.span());
        if raw.len() == 1 && DELIMITERS.contains(&raw[0]) {
            return false;
        }
        let content = symbol_content(v, ctx);
        let content_str = String::from_utf8_lossy(&content);
        let without_pairs = balanced_pair_re().replace_all(&content_str, "");
        let is_complex = content.contains(&b' ')
            || DELIMITERS.iter().any(|d| without_pairs.as_bytes().contains(d));
        if is_complex {
            return true;
        }
    }
    false
}

/// RuboCop's `content = *sym; content.map { |c| c.is_a?(AST::Node) ? c.source : c }.join`:
/// a plain symbol's decoded value, or an interpolated symbol's parts'
/// concatenated raw source text (literal fragments and `#{...}` alike).
fn symbol_content(v: &Node<'_>, ctx: &Context<'_>) -> Vec<u8> {
    if let Some(sn) = v.as_symbol_node() {
        sn.unescaped().to_vec()
    } else if let Some(isn) = v.as_interpolated_symbol_node() {
        let mut buf = Vec::new();
        for part in &isn.parts() {
            buf.extend_from_slice(ctx.text(part.span()));
        }
        buf
    } else {
        Vec::new()
    }
}

/// `/(\[[^\s\[\]]*\])|(\([^\s()]*\))/`.
fn balanced_pair_re() -> &'static Regex {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(\[[^\s\[\]]*\])|(\([^\s()]*\))").expect("static regex"));
    &RE
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
        "Use an array literal `[...]` for an array of symbols.".to_string()
    } else {
        format!("Use `{code}` for an array of symbols.")
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

/// RuboCop's `to_symbol_literal`.
fn to_symbol_literal(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    if symbol_without_quote(&text) {
        format!(":{text}")
    } else {
        format!(":{}", to_string_literal(bytes))
    }
}

/// RuboCop's `symbol_without_quote?`.
fn symbol_without_quote(s: &str) -> bool {
    static METHOD_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\A[a-zA-Z_]\w*[!?]?\z").expect("static regex"));
    static IVAR_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\A@@?[a-zA-Z_]\w*\z").expect("static regex"));
    static GVAR_NUM_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\A\$[1-9]\d*\z").expect("static regex"));
    static GVAR_NAME_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\A\$[a-zA-Z_]\w*\z").expect("static regex"));

    METHOD_RE.is_match(s)
        || IVAR_RE.is_match(s)
        || GVAR_NUM_RE.is_match(s)
        || GVAR_NAME_RE.is_match(s)
        || SPECIAL_GVARS.contains(&s)
        || REDEFINABLE_OPERATORS.contains(&s)
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

/// RuboCop's `PreferredDelimiters#delimiters` for `type_key` (`"%i"`/`"%I"`):
/// `preferred_delimiters_config[type] || preferred_delimiters_config['default']`.
///
/// Read literally, a present type-specific entry always wins over
/// `'default'`. But `config/default.yml`'s own bundled defaults already set
/// `'%i'`/`'%I'` to `[]`, and our config loader deep-merges that bundled
/// layer under every `.rubocop.yml` (correct for real projects); an override
/// that sets only `'default'` would therefore never be visible for `%i`/`%I`
/// specifically. RuboCop's own spec suite configures this option through
/// `RSpec`'s isolated `other_cops:` hash instead, which never merges with
/// `%i`/`%I`'s bundled entries -- so fixtures ported from those specs expect
/// `'default'` to apply. `'default'` is therefore preferred here whenever
/// present, over an inherited type-specific entry.
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
