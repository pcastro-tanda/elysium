//! `Style/StringLiteralsInInterpolation`, ported from RuboCop's
//! `lib/rubocop/cop/style/string_literals_in_interpolation.rb` plus its
//! `StringLiteralsHelp`/`StringHelp` mixins and the shared
//! `StringLiteralCorrector`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// RuboCop's `message` when `style == :single_quotes`.
const MSG_SINGLE: &str = "Prefer single-quoted strings inside interpolations.";
/// RuboCop's `message` when `style == :double_quotes`.
const MSG_DOUBLE: &str = "Prefer double-quoted strings inside interpolations.";

/// The configured `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Single,
    Double,
}

fn message(style: Style) -> &'static str {
    match style {
        Style::Single => MSG_SINGLE,
        Style::Double => MSG_DOUBLE,
    }
}

/// Checks if uses of quotes inside expressions in interpolated strings match
/// the configured preference.
#[derive(Debug, Clone)]
pub struct StringLiteralsInInterpolation {
    style: Style,
    /// Nesting depth inside `#{...}` (Prism's `EmbeddedStatementsNode`).
    embed_depth: u32,
    /// Nesting depth inside a `dstr`/`dsym`/`regexp` equivalent (Prism's
    /// `InterpolatedStringNode`/`InterpolatedSymbolNode`/
    /// `InterpolatedRegularExpressionNode`).
    interp_depth: u32,
}

impl Rule for StringLiteralsInInterpolation {
    const META: RuleMeta = RuleMeta {
        name: "Style/StringLiteralsInInterpolation",
        department: Department::Style,
        summary: "Checks if uses of quotes inside expressions in interpolated strings match the configured preference.",
        explanation: "\
Checks that quotes inside string, symbol, and regexp interpolations match
the configured preference.

```ruby
# EnforcedStyle: single_quotes (default)

# bad
string = \"Tests #{success ? \"PASS\" : \"FAIL\"}\"
symbol = :\"Tests #{success ? \"PASS\" : \"FAIL\"}\"
regexp = /Tests #{success ? \"PASS\" : \"FAIL\"}/

# good
string = \"Tests #{success ? 'PASS' : 'FAIL'}\"
symbol = :\"Tests #{success ? 'PASS' : 'FAIL'}\"
regexp = /Tests #{success ? 'PASS' : 'FAIL'}/
```

```ruby
# EnforcedStyle: double_quotes

# bad
string = \"Tests #{success ? 'PASS' : 'FAIL'}\"
symbol = :\"Tests #{success ? 'PASS' : 'FAIL'}\"
regexp = /Tests #{success ? 'PASS' : 'FAIL'}/

# good
string = \"Tests #{success ? \"PASS\" : \"FAIL\"}\"
symbol = :\"Tests #{success ? \"PASS\" : \"FAIL\"}\"
regexp = /Tests #{success ? \"PASS\" : \"FAIL\"}/
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::StringNode,
            NodeKind::InterpolatedStringNode,
            NodeKind::EmbeddedStatementsNode,
            NodeKind::InterpolatedSymbolNode,
            NodeKind::InterpolatedRegularExpressionNode,
        ],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("single_quotes"),
            allowed: &["single_quotes", "double_quotes"],
            doc: "Preferred string literal quote style for interpolated expressions.",
        }],
        blind_spots: "\
Ports whitequark's `inside_interpolation?` with nesting counters rather than
a true ancestor walk (matching `Style/StringLiterals`'s port), so it does
not distinguish further by container kind beyond string/symbol/regexp vs.
everything else, which matches RuboCop's own behaviour.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = if options.style("EnforcedStyle")? == "double_quotes" {
            Style::Double
        } else {
            Style::Single
        };
        Ok(Self { style, embed_depth: 0, interp_depth: 0 })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.embed_depth = 0;
        self.interp_depth = 0;
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::EmbeddedStatementsNode { .. } => self.embed_depth += 1,
            Node::InterpolatedStringNode { .. }
            | Node::InterpolatedSymbolNode { .. }
            | Node::InterpolatedRegularExpressionNode { .. } => {
                self.interp_depth += 1;
            }
            Node::StringNode { .. } if self.inside_interpolation() => {
                let n = node.as_string_node().expect("kind matched");
                self.check_string(&n, ctx);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node {
            Node::EmbeddedStatementsNode { .. } => self.embed_depth -= 1,
            Node::InterpolatedStringNode { .. }
            | Node::InterpolatedSymbolNode { .. }
            | Node::InterpolatedRegularExpressionNode { .. } => {
                self.interp_depth -= 1;
            }
            _ => {}
        }
    }
}

impl StringLiteralsInInterpolation {
    /// RuboCop's `StringHelp#inside_interpolation?`: true for a string
    /// literal nested inside a `#{...}` interpolation of an enclosing
    /// `dstr`/`dsym`/`regexp`.
    fn inside_interpolation(&self) -> bool {
        self.embed_depth > 0 && self.interp_depth > 0
    }

    /// RuboCop's `StringHelp#on_str` plus `StringLiteralsInInterpolation#offense?`.
    fn check_string(&self, node: &ruby_ast::node::StringNode<'_>, ctx: &mut Context<'_>) {
        let Some(opening) = node.opening_loc() else { return };
        let opening_text = ctx.text(opening.span());
        if opening_text.starts_with(b"<<") {
            return; // heredoc: RuboCop's `str`/`dstr` nodes never carry a `begin` loc.
        }

        let span = node.location().span();
        let src = ctx.text(span);

        if wrong_quotes(src, self.style) {
            ctx.report_with_fix(
                &Self::META,
                span,
                message(self.style),
                build_fix(node, self.style),
            );
        }
    }
}

/// RuboCop's `Util#double_quotes_required?`: true when `src` (raw source
/// text, including its own delimiters) contains a literal `'`, or a
/// backslash whose maximal run has odd length and is not immediately
/// followed by `"` -- i.e. an escape sequence a single-quoted literal
/// cannot represent losslessly.
fn double_quotes_required(src: &[u8]) -> bool {
    if src.contains(&b'\'') {
        return true;
    }
    let mut i = 0;
    while i < src.len() {
        if src[i] == b'\\' {
            let start = i;
            while i < src.len() && src[i] == b'\\' {
                i += 1;
            }
            let run_len = i - start;
            if run_len % 2 == 1 && src.get(i) != Some(&b'"') {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

/// The inline regex in RuboCop's `StringLiteralsHelp#wrong_quotes?`'s
/// `style == :double_quotes` branch: true when `src` (raw single-quoted
/// source, including its own delimiters) must stay single-quoted because it
/// has a raw `"`, a backslash escape other than `\\` or `\'`, or an
/// interpolation marker (`#{`, `#@`, `#$`).
fn must_stay_single_quoted(src: &[u8]) -> bool {
    let mut i = 0;
    while i < src.len() {
        match src[i] {
            b'"' => return true,
            b'\\' => {
                if let Some(&next) = src.get(i + 1) {
                    if next != b'\'' && next != b'\\' {
                        return true;
                    }
                }
            }
            b'#' => {
                if matches!(src.get(i + 1), Some(b'{' | b'@' | b'$')) {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// RuboCop's `StringLiteralsHelp#wrong_quotes?`.
fn wrong_quotes(src: &[u8], style: Style) -> bool {
    if matches!(src.first(), Some(b'%' | b'?')) {
        return false;
    }
    match style {
        Style::Single => !double_quotes_required(src),
        Style::Double => !must_stay_single_quoted(src),
    }
}

/// RuboCop's `Util#to_string_literal` single-quoted branch, specialised for
/// values that can legally trigger this cop's single-quotes offense: since
/// `wrong_quotes?` (style `single_quotes`) requires no raw `'` and only
/// `\\`-paired backslashes in the source, the decoded value can never
/// contain an apostrophe, so the `needs_escaping?` fallback to `.inspect`
/// never triggers here. Doubles every backslash, then folds `\"` sequences
/// (produced when a backslash immediately precedes a literal `"`) back to a
/// bare `"`, since single-quoted strings never need to escape `"`.
fn single_quote_escape(value: &[u8]) -> Vec<u8> {
    let mut doubled = Vec::with_capacity(value.len() * 2);
    for &b in value {
        if b == b'\\' {
            doubled.push(b'\\');
        }
        doubled.push(b);
    }
    let mut out = Vec::with_capacity(doubled.len());
    let mut i = 0;
    while i < doubled.len() {
        if doubled[i] == b'\\' && doubled.get(i + 1) == Some(&b'"') {
            out.push(b'"');
            i += 2;
        } else {
            out.push(doubled[i]);
            i += 1;
        }
    }
    out
}

/// RuboCop's `Util#to_string_literal` double-quoted branch (`str.inspect`),
/// specialised the same way: `wrong_quotes?` (style `double_quotes`)
/// requires no raw `"` and only `\\`/`\'` backslash escapes in the source,
/// so the decoded value never needs anything beyond escaping a literal `\`
/// or `"`.
fn double_quote_escape(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len());
    for &b in value {
        match b {
            b'\\' => {
                out.push(b'\\');
                out.push(b'\\');
            }
            b'"' => {
                out.push(b'\\');
                out.push(b'"');
            }
            _ => out.push(b),
        }
    }
    out
}

/// RuboCop's `StringLiteralCorrector.correct` for a plain (non-`dstr`) `str`
/// node.
fn build_fix(node: &ruby_ast::node::StringNode<'_>, style: Style) -> Fix {
    let value = node.unescaped();
    let mut replacement = Vec::with_capacity(value.len() + 2);
    match style {
        Style::Single => {
            replacement.push(b'\'');
            replacement.extend(single_quote_escape(value));
            replacement.push(b'\'');
        }
        Style::Double => {
            replacement.push(b'"');
            replacement.extend(double_quote_escape(value));
            replacement.push(b'"');
        }
    }
    Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::replace(node.location().span(), replacement)],
    }
}
