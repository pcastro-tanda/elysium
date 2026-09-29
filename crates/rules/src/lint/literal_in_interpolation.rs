//! `Lint/LiteralInInterpolation`, ported from RuboCop's
//! `lib/rubocop/cop/lint/literal_in_interpolation.rb` plus the `Interpolation`
//! and `PercentLiteral` mixins it includes.
//!
//! Whitequark's `begin` node (the `#{...}` wrapper) has the interpolated
//! expression(s) directly as children; Prism instead wraps them in an extra
//! `StatementsNode` inside `EmbeddedStatementsNode`. So where upstream reads
//! `begin_node.children.last` / `node.parent` / `node.parent.parent`, this
//! port reads `EmbeddedStatementsNode::statements().body().last()` for the
//! final expression, and uses `ctx.parent()` (the ancestor stack) for what
//! upstream calls the interpolation's "grandparent" -- the `Interpolated*Node`
//! container one level above the `EmbeddedStatementsNode` being visited.
//!
//! Whitequark also reparses `__FILE__`/`__LINE__`/`__ENCODING__`/`__END__`
//! used inside interpolation as ordinary `str`/`int`/`const`/`send` nodes
//! (hence the `special_keyword?` guard upstream needs); Prism gives these
//! their own dedicated node kinds (`SourceFileNode`, `SourceLineNode`,
//! `SourceEncodingNode`, plus a plain `CallNode` for `__END__`), none of
//! which match this cop's literal-node kinds, so no equivalent guard is
//! needed here.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, NodeInfo, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Literal interpolation detected.";

/// Checks for literals used in interpolation.
#[derive(Debug, Clone)]
pub struct LiteralInInterpolation;

impl Rule for LiteralInInterpolation {
    const META: RuleMeta = RuleMeta {
        name: "Lint/LiteralInInterpolation",
        department: Department::Lint,
        summary: "Checks for literals used in interpolation.",
        explanation: "\
Checks for interpolated literals.

NOTE: Array literals interpolated in regexps are not handled by this cop,
but by `Lint/ArrayLiteralInRegexp` instead.

```ruby
# bad
\"result is #{10}\"

# good
\"result is 10\"
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::EmbeddedStatementsNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(begin) = node.as_embedded_statements_node() else { return };
        let Some(statements) = begin.statements() else { return };
        let Some(final_node) = statements.body().last() else { return };

        if !offending(ctx, &final_node) {
            return;
        }

        let container = ctx.parent();

        let mut expanded = autocorrected_value(ctx, &final_node);
        if let Some(container) = container {
            expanded = handle_special_regexp_chars(ctx, container, expanded);
        }

        if let Some(container) = container {
            if is_in_array_percent_literal(ctx, container)
                && (expanded.is_empty() || expanded.iter().copied().any(is_ruby_space))
            {
                return;
            }
        }

        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node.span(), expanded)],
        };
        ctx.report_with_fix(&Self::META, final_node.span(), MSG, fix);
    }
}

/// Ruby's `\s` regexp class: space, tab, newline, CR, form feed, vertical tab.
fn is_ruby_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

/// RuboCop's `offending?`, minus the `special_keyword?` guard (see the module
/// doc comment: Prism never reaches this method with a node it would have
/// mattered for).
fn offending(ctx: &Context<'_>, node: &Node<'_>) -> bool {
    prints_as_self(node)
        && !(is_space_literal(node) && ends_heredoc_line(ctx, node))
        && !is_array_in_regexp(ctx, node)
}

/// RuboCop's `array_in_regexp?`: `node` is an array literal directly
/// interpolated into a regexp (handled by `Lint/ArrayLiteralInRegexp`
/// instead). `ctx.parent()` here is the `Interpolated*Node` container one
/// level above the `EmbeddedStatementsNode` currently being visited -- what
/// upstream reaches as `node.parent.parent`.
fn is_array_in_regexp(ctx: &Context<'_>, node: &Node<'_>) -> bool {
    node.kind() == NodeKind::ArrayNode
        && ctx.parent().is_some_and(|p| p.kind == NodeKind::InterpolatedRegularExpressionNode)
}

/// RuboCop's `space_literal?`.
fn is_space_literal(node: &Node<'_>) -> bool {
    node.as_string_node().is_some_and(|s| {
        let value = s.unescaped();
        std::str::from_utf8(value).is_ok() && value.iter().all(|&b| b.is_ascii_whitespace())
    })
}

/// RuboCop's `ends_heredoc_line?`.
fn ends_heredoc_line(ctx: &Context<'_>, node: &Node<'_>) -> bool {
    let Some(container) = ctx.parent() else { return false };
    if container.kind != NodeKind::InterpolatedStringNode {
        return false;
    }
    if !ctx.text(container.span).starts_with(b"<<") {
        return false;
    }
    let last_line = ctx.last_line(node.span());
    ctx.line_span(last_line).end == node.span().end + 1
}

/// RuboCop's `in_array_percent_literal?`. `container` is what upstream calls
/// `begin_node.parent`; its own parent (one more level up the ancestor
/// stack) is the candidate `%w`/`%W`/`%i`/`%I` array.
fn is_in_array_percent_literal(ctx: &Context<'_>, container: NodeInfo) -> bool {
    if !matches!(
        container.kind,
        NodeKind::InterpolatedStringNode | NodeKind::InterpolatedSymbolNode
    ) {
        return false;
    }
    let ancestors = ctx.ancestors();
    let Some(grandparent) = ancestors.len().checked_sub(2).and_then(|i| ancestors.get(i)) else {
        return false;
    };
    grandparent.kind == NodeKind::ArrayNode && is_percent_literal_span(ctx, grandparent.span)
}

/// RuboCop's `PercentLiteral#percent_literal?`: the node's own opening
/// delimiter starts with `%`.
fn is_percent_literal_span(ctx: &Context<'_>, span: Span) -> bool {
    ctx.text(Span::new(span.start, span.start + 1)) == b"%"
}

/// RuboCop's `prints_as_self?`: does `node` print its own source when
/// converted to a string?
fn prints_as_self(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::IntegerNode
        | NodeKind::FloatNode
        | NodeKind::StringNode
        | NodeKind::SymbolNode
        | NodeKind::TrueNode
        | NodeKind::FalseNode
        | NodeKind::NilNode
        | NodeKind::ImaginaryNode
        | NodeKind::RationalNode => true,
        NodeKind::ArrayNode => {
            node.as_array_node().is_some_and(|a| a.elements().iter().all(|c| prints_as_self(&c)))
        }
        NodeKind::HashNode => {
            node.as_hash_node().is_some_and(|h| h.elements().iter().all(|c| prints_as_self(&c)))
        }
        NodeKind::AssocNode => node
            .as_assoc_node()
            .is_some_and(|a| prints_as_self(&a.key()) && prints_as_self(&a.value())),
        NodeKind::RangeNode => node.as_range_node().is_some_and(|r| {
            r.left().is_some_and(|l| prints_as_self(&l))
                && r.right().is_some_and(|r| prints_as_self(&r))
        }),
        _ => false,
    }
}

/// RuboCop's `autocorrected_value` (the top-level, non-hash-nested variant).
fn autocorrected_value(ctx: &Context<'_>, node: &Node<'_>) -> Vec<u8> {
    match node.kind() {
        NodeKind::IntegerNode => integer_to_decimal(ctx, node),
        NodeKind::FloatNode => float_to_ruby_string(node),
        NodeKind::StringNode => autocorrected_value_for_string(ctx, node),
        NodeKind::SymbolNode => autocorrected_value_for_symbol(ctx, node),
        NodeKind::ArrayNode => autocorrected_value_for_array(ctx, node),
        NodeKind::HashNode => autocorrected_value_for_hash(ctx, node),
        NodeKind::NilNode => Vec::new(),
        _ => escape_quotes_only(ctx.text(node.span())),
    }
}

/// RuboCop's `autocorrected_value_in_hash`: the same dispatch, but literals
/// nested inside a hash render themselves through Ruby's own `inspect`
/// (matching what `Hash#to_s` would print) rather than through the
/// top-level, string-content-only rules.
fn autocorrected_value_in_hash(ctx: &Context<'_>, node: &Node<'_>) -> Vec<u8> {
    match node.kind() {
        NodeKind::IntegerNode => integer_to_decimal(ctx, node),
        NodeKind::FloatNode => float_to_ruby_string(node),
        NodeKind::StringNode => {
            let unescaped =
                node.as_string_node().map(|s| s.unescaped().to_vec()).unwrap_or_default();
            escape_string_content(&ruby_inspect_string(&unescaped))
        }
        NodeKind::SymbolNode => {
            let unescaped =
                node.as_symbol_node().map(|s| s.unescaped().to_vec()).unwrap_or_default();
            escape_string_content(&ruby_symbol_inspect(&unescaped))
        }
        NodeKind::ArrayNode => autocorrected_value_for_array(ctx, node),
        NodeKind::HashNode => autocorrected_value_for_hash(ctx, node),
        _ => escape_quotes_only(ctx.text(node.span())),
    }
}

/// RuboCop's `autocorrected_value_for_string`.
fn autocorrected_value_for_string(ctx: &Context<'_>, node: &Node<'_>) -> Vec<u8> {
    let Some(s) = node.as_string_node() else { return Vec::new() };
    let unescaped = s.unescaped();
    if std::str::from_utf8(unescaped).is_err() {
        return ctx.text(s.content_loc().span()).to_vec();
    }
    escape_string_content(unescaped)
}

/// RuboCop's `autocorrected_value_for_symbol`.
fn autocorrected_value_for_symbol(ctx: &Context<'_>, node: &Node<'_>) -> Vec<u8> {
    let Some(s) = node.as_symbol_node() else { return Vec::new() };
    let raw = match s.value_loc() {
        Some(loc) => ctx.text(loc.span()),
        None => ctx.text(node.span()),
    };
    escape_quotes_only(raw)
}

/// RuboCop's `autocorrected_value_for_array`.
fn autocorrected_value_for_array(ctx: &Context<'_>, node: &Node<'_>) -> Vec<u8> {
    let Some(arr) = node.as_array_node() else { return Vec::new() };
    let opening = arr.opening_loc();
    let is_percent = opening.as_ref().is_some_and(|l| ctx.text(l.span()).starts_with(b"%"));
    if !is_percent {
        return escape_quotes_only(ctx.text(node.span()));
    }
    let Some(opening) = opening else { return escape_quotes_only(ctx.text(node.span())) };
    let start = opening.span().end;
    let end = arr.closing_loc().map_or(node.span().end, |l| l.span().start);
    let contents = String::from_utf8_lossy(ctx.text(Span::new(start, end)));
    let words: Vec<String> = contents.split_whitespace().map(|w| format!("\"{w}\"")).collect();
    let rendered = format!("[{}]", words.join(", "));
    escape_quotes_only(rendered.as_bytes())
}

/// RuboCop's `autocorrected_value_for_hash`.
fn autocorrected_value_for_hash(ctx: &Context<'_>, node: &Node<'_>) -> Vec<u8> {
    let Some(h) = node.as_hash_node() else { return Vec::new() };
    let mut out = vec![b'{'];
    let mut first = true;
    for child in &h.elements() {
        let Some(assoc) = child.as_assoc_node() else { continue };
        if !first {
            out.extend_from_slice(b", ");
        }
        first = false;
        out.extend_from_slice(&autocorrected_value_in_hash(ctx, &assoc.key()));
        out.extend_from_slice(b"=>");
        out.extend_from_slice(&autocorrected_value_in_hash(ctx, &assoc.value()));
    }
    out.push(b'}');
    out
}

/// RuboCop's `escape_string_content`:
/// `string.gsub(/[\\"]|#(?=[@{$])/, '\\\\\&')`.
fn escape_string_content(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' || b == b'"' {
            out.push(b'\\');
            out.push(b);
        } else if b == b'#' && matches!(bytes.get(i + 1), Some(b'@' | b'{' | b'$')) {
            out.push(b'\\');
            out.push(b'#');
        } else {
            out.push(b);
        }
        i += 1;
    }
    out
}

/// `.gsub('"', '\"')`: only the quote character is escaped, backslashes are
/// left untouched.
fn escape_quotes_only(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        if b == b'"' {
            out.push(b'\\');
        }
        out.push(b);
    }
    out
}

/// Ruby's `String#inspect`: double-quoted, with the usual backslash escapes.
/// Only the escapes this cop's fixtures can ever produce are implemented
/// (plain ASCII content plus the handful of control characters Ruby names);
/// anything else falls back to a `\xHH` byte escape.
fn ruby_inspect_string(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![b'"'];
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'"' => out.extend_from_slice(b"\\\""),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\t' => out.extend_from_slice(b"\\t"),
            b'\r' => out.extend_from_slice(b"\\r"),
            0x1B => out.extend_from_slice(b"\\e"),
            0x08 => out.extend_from_slice(b"\\b"),
            0x0C => out.extend_from_slice(b"\\f"),
            0x0B => out.extend_from_slice(b"\\v"),
            0x07 => out.extend_from_slice(b"\\a"),
            0x00 => out.extend_from_slice(b"\\0"),
            b'#' if matches!(bytes.get(i + 1), Some(b'@' | b'{' | b'$')) => {
                out.extend_from_slice(b"\\#");
            }
            b if b < 0x20 || b == 0x7F => {
                out.extend_from_slice(format!("\\x{b:02X}").as_bytes());
            }
            b => out.push(b),
        }
        i += 1;
    }
    out.push(b'"');
    out
}

/// Ruby's `Symbol#inspect`: bare `:name` when `name` is a simple
/// identifier/operator method name, otherwise `:` followed by
/// `name.inspect`.
fn ruby_symbol_inspect(name: &[u8]) -> Vec<u8> {
    if symbol_is_simple(name) {
        let mut out = vec![b':'];
        out.extend_from_slice(name);
        out
    } else {
        let mut out = vec![b':'];
        out.extend_from_slice(&ruby_inspect_string(name));
        out
    }
}

/// Whether `name` prints bare (without quotes) as a `Symbol#inspect`.
/// Covers plain identifiers/setters/predicates, `$`/`@`/`@@`-prefixed
/// variable names, and the fixed set of operator method names; anything
/// else (spaces, punctuation, empty) needs quoting.
fn symbol_is_simple(name: &[u8]) -> bool {
    const OPERATORS: &[&str] = &[
        "+", "-", "*", "**", "/", "%", "~", "!", "==", "===", "!=", "<", "<=", ">", ">=", "<=>",
        "<<", ">>", "&", "|", "^", "=~", "!~", "[]", "[]=", "+@", "-@", "`",
    ];
    let Ok(s) = std::str::from_utf8(name) else { return false };
    if s.is_empty() {
        return false;
    }
    if OPERATORS.contains(&s) {
        return true;
    }
    let rest = if let Some(r) = s.strip_prefix('$') {
        r
    } else if let Some(r) = s.strip_prefix("@@") {
        r
    } else if let Some(r) = s.strip_prefix('@') {
        r
    } else {
        s
    };
    let mut chars = rest.chars();
    let Some(head) = chars.next() else { return false };
    if !(head.is_alphabetic() || head == '_') {
        return false;
    }
    let body = &rest[head.len_utf8()..];
    let body = body.strip_suffix(['?', '!', '=']).unwrap_or(body);
    body.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Re-parses the integer literal's own source text (stripping `_` digit
/// separators and resolving the `0x`/`0o`/`0b`/`0d`/leading-`0` radix
/// prefixes), matching RuboCop's `node.children.last.to_i.to_s`. `i128` is
/// used instead of Ruby's arbitrary-precision `Integer`; every fixture value
/// fits comfortably within it.
fn integer_to_decimal(ctx: &Context<'_>, node: &Node<'_>) -> Vec<u8> {
    let text = std::str::from_utf8(ctx.text(node.span())).unwrap_or("0");
    let (negative, rest) = match text.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let cleaned = rest.replace('_', "");
    let (radix, digits): (u32, &str) =
        if let Some(d) = cleaned.strip_prefix("0x").or_else(|| cleaned.strip_prefix("0X")) {
            (16, d)
        } else if let Some(d) = cleaned.strip_prefix("0b").or_else(|| cleaned.strip_prefix("0B")) {
            (2, d)
        } else if let Some(d) = cleaned.strip_prefix("0o").or_else(|| cleaned.strip_prefix("0O")) {
            (8, d)
        } else if let Some(d) = cleaned.strip_prefix("0d").or_else(|| cleaned.strip_prefix("0D")) {
            (10, d)
        } else if cleaned.len() > 1 && cleaned.starts_with('0') {
            (8, &cleaned[1..])
        } else {
            (10, cleaned.as_str())
        };
    let value = i128::from_str_radix(digits, radix).unwrap_or(0);
    let value = if negative { -value } else { value };
    value.to_string().into_bytes()
}

/// RuboCop's `node.children.last.to_f.to_s`.
fn float_to_ruby_string(node: &Node<'_>) -> Vec<u8> {
    let value = node.as_float_node().map_or(0.0, |f| f.value());
    format!("{value:?}").into_bytes()
}

/// RuboCop's `handle_special_regexp_chars`. `container` is the
/// `Interpolated*Node` one level above the `EmbeddedStatementsNode`
/// currently being visited -- upstream's `begin_node.parent`.
fn handle_special_regexp_chars(ctx: &Context<'_>, container: NodeInfo, value: Vec<u8>) -> Vec<u8> {
    if container.kind != NodeKind::InterpolatedRegularExpressionNode {
        return value;
    }
    if ctx.text(Span::new(container.span.start, container.span.start + 1)) != b"/" {
        return value;
    }
    if !value.contains(&b'/') {
        return value;
    }
    escape_slashes_for_regexp(&value)
}

/// The `value.gsub(%r{(\\*)/}) { ... }` substitution: each maximal run of
/// backslashes immediately followed by `/` is replaced by
/// `2 * ((backslash_count + 1) / 4) + 1` backslashes plus the `/`.
fn escape_slashes_for_regexp(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len());
    let mut i = 0;
    while i < value.len() {
        if value[i] == b'\\' {
            let start = i;
            while i < value.len() && value[i] == b'\\' {
                i += 1;
            }
            if i < value.len() && value[i] == b'/' {
                let backslash_count = i - start;
                let needed = 2 * ((backslash_count + 1) / 4) + 1;
                out.extend(std::iter::repeat_n(b'\\', needed));
                out.push(b'/');
                i += 1;
            } else {
                out.extend_from_slice(&value[start..i]);
            }
        } else if value[i] == b'/' {
            out.push(b'\\');
            out.push(b'/');
            i += 1;
        } else {
            out.push(value[i]);
            i += 1;
        }
    }
    out
}
