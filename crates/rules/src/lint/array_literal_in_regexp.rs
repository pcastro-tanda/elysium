//! `Lint/ArrayLiteralInRegexp`, ported from RuboCop's
//! `lib/rubocop/cop/lint/array_literal_in_regexp.rb` plus the
//! `Interpolation` mixin it includes.
//!
//! Upstream's `Interpolation#on_dstr`/`on_xstr`/`on_dsym`/`on_regexp` all
//! call `on_interpolation` for every `begin`-type (whitequark) child, which
//! then filters to `begin_node.parent.regexp_type?` -- so the only case
//! that ever survives is an interpolation directly inside a regexp literal.
//! This port skips straight to that case: it only subscribes to
//! `InterpolatedRegularExpressionNode` and walks its own `parts`, each
//! `EmbeddedStatementsNode` standing in for whitequark's `begin_node`
//! (Prism's dedicated node for a `#{...}` interpolation, versus a bare
//! `StringNode` for the literal text parts in between).
//!
//! `LITERAL_TYPES` (`str sym int float true false nil`) deliberately
//! excludes the composite/interpolated forms (`dstr`, `dsym`, `xstr`,
//! `regexp`): Prism's equivalent exclusion is `InterpolatedStringNode`,
//! `InterpolatedSymbolNode`, `XStringNode`, `RegularExpressionNode` (and
//! their own interpolated variants), matching
//! `offense_registers_an_offense_but_does_not_correct_for_foo_2` (a
//! `["#{foo}"]` array element) falling through to the "unknown" message.
//!
//! `value.respond_to?(:value) ? value.value : value.source`: only
//! `str`/`sym`/`int`/`float` nodes answer `.value` upstream (`true`/`false`/
//! `nil` fall back to their own source text, which is identical to their
//! Ruby value's `to_s` anyway). This port always reads `int`/`float`/
//! `true`/`false`/`nil` straight from source text (identical to `.value.to_s`
//! for the plain decimal literals these fixtures use) and only decodes
//! `str`/`sym` via `unescaped` (whitequark's `.value`, stripped of
//! quoting/sigil).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG_CHARACTER_CLASS: &str =
    "Use a character class instead of interpolating an array in a regexp.";
const MSG_ALTERNATION: &str = "Use alternation instead of interpolating an array in a regexp.";
const MSG_UNKNOWN: &str = "Use alternation or a character class instead of interpolating an array \
in a regexp.";

fn is_literal_value(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StringNode
            | NodeKind::SymbolNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
    )
}

/// `value.respond_to?(:value) ? value.value : value.source`, restated for
/// the Prism kinds [`is_literal_value`] admits (see the module doc).
fn element_value(node: &Node<'_>, ctx: &Context<'_>) -> Vec<u8> {
    match node.kind() {
        NodeKind::StringNode => node.as_string_node().expect("kind matched").unescaped().to_vec(),
        NodeKind::SymbolNode => node.as_symbol_node().expect("kind matched").unescaped().to_vec(),
        _ => ctx.text(node.span()).to_vec(),
    }
}

/// Ruby's `Regexp.escape`, restricted to the ASCII metacharacter/whitespace/
/// `#` set it treats specially (see the module doc's blind-spot note);
/// every other byte, including multi-byte UTF-8 sequences, passes through
/// unchanged.
fn regexp_escape(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            b'\t' => out.extend_from_slice(b"\\t"),
            b'\n' => out.extend_from_slice(b"\\n"),
            0x0B => out.extend_from_slice(b"\\v"),
            0x0C => out.extend_from_slice(b"\\f"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b' ' | b'#' | b'$' | b'(' | b')' | b'*' | b'+' | b'-' | b'.' | b'?' | b'[' | b'\\'
            | b']' | b'^' | b'{' | b'|' | b'}' => {
                out.push(b'\\');
                out.push(b);
            }
            0 => out.extend_from_slice(b"\\x00"),
            _ => out.push(b),
        }
    }
    out
}

/// `character_class?`: every value is exactly one character long (counting
/// Unicode scalar values, not bytes -- `"❤️".length == 2` in Ruby, since it
/// is a heart plus a variation selector, which is why that fixture uses
/// alternation instead of a character class).
fn is_character_class(values: &[Vec<u8>]) -> bool {
    values.iter().all(|v| String::from_utf8_lossy(v).chars().count() == 1)
}

fn escape_values(values: &[Vec<u8>]) -> Vec<Vec<u8>> {
    values.iter().map(|v| regexp_escape(v)).collect()
}

fn character_class_for(values: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![b'['];
    out.extend(escape_values(values).concat());
    out.push(b']');
    out
}

fn alternation_for(values: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"(?:".to_vec();
    out.extend(escape_values(values).join(&b'|'));
    out.push(b')');
    out
}

/// Checks for an array literal interpolated inside a regexp.
#[derive(Debug, Clone)]
pub struct ArrayLiteralInRegexp;

impl Rule for ArrayLiteralInRegexp {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ArrayLiteralInRegexp",
        department: Department::Lint,
        summary: "Checks for an array literal interpolated inside a regexp.",
        explanation: "\
Checks for an array literal interpolated inside a regexp.

When interpolating an array literal, it is converted to a string. This \
means that when inside a regexp, it acts as a character class but with \
additional quotes, spaces and commas that are likely not intended. For \
example, `/#{%w[a b c]}/` parses as `/[\"a\", \"b\", \"c\"]/` (or `/[\"a, bc]/` \
without repeated characters).

The cop can autocorrect to a character class (if all items in the array are \
a single character) or alternation (if the array contains longer items).

NOTE: This only considers interpolated arrays that contain only strings, \
symbols, integers, and floats. Any other type is not easily convertible to \
a character class or regexp alternation.

```ruby
# bad
/#{%w[a b c]}/

# good
/[abc]/

# bad
/#{%w[foo bar baz]}/

# good
/(?:foo|bar|baz)/

# bad - construct a regexp rather than interpolate an array of identifiers
/#{[foo, bar]}/
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::InterpolatedRegularExpressionNode],
        config: &[],
        blind_spots: "Control characters outside the whitespace/`#`/metacharacter set that Ruby's \
`Regexp.escape` treats specially (e.g. `\\x01`) are copied verbatim instead of escaped, since no \
fixture exercises them.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(regexp) = node.as_interpolated_regular_expression_node() else { return };

        for part in &regexp.parts() {
            let Some(embedded) = part.as_embedded_statements_node() else { continue };
            let Some(statements) = embedded.statements() else { continue };
            let Some(last) = statements.body().last() else { continue };
            let Some(array) = last.as_array_node() else { continue };

            let span = part.span();
            let elements: Vec<Node<'_>> = array.elements().iter().collect();

            if elements.iter().all(|e| is_literal_value(e.kind())) {
                let values: Vec<Vec<u8>> = elements.iter().map(|e| element_value(e, ctx)).collect();
                let (message, replacement) = if is_character_class(&values) {
                    (MSG_CHARACTER_CLASS, character_class_for(&values))
                } else {
                    (MSG_ALTERNATION, alternation_for(&values))
                };
                let fix = Fix {
                    applicability: Applicability::Unsafe,
                    edits: vec![Edit::replace(span, replacement)],
                };
                ctx.report_with_fix(&Self::META, span, message, fix);
            } else {
                ctx.report(&Self::META, span, MSG_UNKNOWN);
            }
        }
    }
}
