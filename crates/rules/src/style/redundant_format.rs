//! `Style/RedundantFormat`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_format.rb` plus the
//! `RuboCop::Cop::Utils::FormatString`/`FormatSequence` helper it relies on.
//!
//! # Format-sequence parsing
//!
//! Upstream's `SEQUENCE` regex allows a `<name>`/`{name}` reference to sit
//! in three different positions relative to width/precision (to tolerate
//! unusual orderings); this port only recognizes the canonical ordering
//! (`%[flags]<name>[width][.precision]type` / `%[width][.precision]{name}`)
//! that every example in this cop's own spec and fixtures uses. It also
//! drops the `TEMPLATE_NAME` negative lookbehind that disambiguates a
//! literal `%#{...}` text run from a `{name}` template (not expressible in
//! the `regex` crate, and not exercised here since the format string this
//! cop inspects is always a plain, non-interpolated `str` literal).
//!
//! # Re-rendering a literal format call
//!
//! Upstream's `register_all_fields_literal` validates field-by-field
//! against a *duplicated, consumed* copy of the argument list, then simply
//! calls Ruby's own `Kernel#format` on the *original* list to produce the
//! replacement text. With no `Kernel#format` to call, this port instead
//! implements the handful of directives the validation step ever accepts
//! (`%s`, `%d`/`%i`/`%u`, `%f`, with `-0+# ` flags and fixed/variable
//! width/precision) directly: [`all_fields_literal`] mirrors the
//! validation (consuming a scratch copy, failing the moment any sequence
//! does not match -- a bare `%%` escape anywhere always fails it, since
//! upstream's `count` is never incremented for a percent sequence while
//! `sequences.size` still counts it), and [`render_format`] walks the
//! *same original* argument list a second time, left to right, consuming a
//! `*` width/precision before the value it belongs to, exactly as Ruby's
//! own formatter would.
//!
//! # Numeric literal values
//!
//! A `Rational`/`Complex` *literal* in Ruby source is actually sugar for a
//! runtime division/addition (`3/8r` parses as `3 / Rational(8, 1)`,
//! `5+0i` as `5 + Complex(0, 1)`); [`literal_rational`]/[`literal_complex`]
//! recognize exactly the two shapes upstream's own `rational_number?`/
//! `complex_number?` node matchers do and fold the arithmetic directly,
//! rather than modelling arbitrary-precision `Rational`/`Complex` values.

use std::sync::LazyLock;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::CallNode;
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
fn message(method_name: &[u8], prefer: &[u8]) -> String {
    format!(
        "Use `{}` directly instead of `{}`.",
        String::from_utf8_lossy(prefer),
        String::from_utf8_lossy(method_name)
    )
}

/// Checks for usages of `Kernel#format` or `Kernel#sprintf` with only a single argument.
#[derive(Debug, Clone)]
pub struct RedundantFormat;

impl Rule for RedundantFormat {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantFormat",
        department: Department::Style,
        summary:
            "Checks for usages of `Kernel#format` or `Kernel#sprintf` with only a single argument.",
        explanation: "\
Calling `format` with only a single string or constant argument is redundant,
as it can be replaced by the string or constant itself.

Also looks for `format` calls where the arguments are literals that can be
inlined into a string easily. This applies to the `%s`, `%d`, `%i`, `%u`, and
`%f` format specifiers.

```ruby
# bad
format('the quick brown fox jumps over the lazy dog.')
sprintf('the quick brown fox jumps over the lazy dog.')

# good
'the quick brown fox jumps over the lazy dog.'

# bad
format(MESSAGE)
sprintf(MESSAGE)

# good
MESSAGE

# bad
format('%s %s', 'foo', 'bar')
sprintf('%s %s', 'foo', 'bar')

# good
'foo bar'
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Format-sequence parsing only recognizes the canonical `<name>`/`{name}`
ordering (right after flags, before width/precision), drops the
`TEMPLATE_NAME` lookbehind that disambiguates a literal `%#{...}` text run
from a `{name}` template, and `Rational`/`Complex` literal values only cover
the exact `int / Nr` and `int + Ni` shapes upstream's own node matchers
recognize (not arbitrary nested arithmetic).",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name().as_slice();
        if name != b"format" && name != b"sprintf" {
            return;
        }
        if !is_nil_or_kernel_receiver(&call) {
            return;
        }

        if let Some(single) = single_literal_arg(&call) {
            if !has_format_sequence(&single) {
                register_single(ctx, node, &single, name);
                return;
            }
        }

        detect_unnecessary_fields(&call, node, ctx, name);
    }
}

/// `{(const {nil? cbase} :Kernel) nil?}`.
fn is_nil_or_kernel_receiver(call: &CallNode<'_>) -> bool {
    match call.receiver() {
        None => true,
        Some(r) => {
            r.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"Kernel")
                || r.as_constant_path_node().is_some_and(|p| {
                    p.parent().is_none() && p.name().is_some_and(|n| n.as_slice() == b"Kernel")
                })
        }
    }
}

/// `format_without_additional_args?`'s captured value: the sole argument,
/// when it is a `str`/`dstr`/const literal.
fn single_literal_arg<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let args = call.arguments()?;
    let mut iter = args.arguments().iter();
    let first = iter.next()?;
    if iter.next().is_some() {
        return None;
    }
    match first.kind() {
        NodeKind::StringNode
        | NodeKind::InterpolatedStringNode
        | NodeKind::ConstantReadNode
        | NodeKind::ConstantPathNode => Some(first),
        _ => None,
    }
}

/// `string_with_format_sequence?`: only a statically-known `str`/
/// all-`str`-children `dstr` value is checked for a format sequence;
/// anything else (a `dstr` with real interpolation, or a const) is treated
/// as sequence-free, so the single-argument offense always fires for those.
fn has_format_sequence(arg: &Node<'_>) -> bool {
    let Some(bytes) = static_string_value(arg) else { return false };
    let Ok(s) = std::str::from_utf8(&bytes) else { return false };
    !format_sequences(s).is_empty()
}

fn static_string_value(arg: &Node<'_>) -> Option<Vec<u8>> {
    if let Some(s) = arg.as_string_node() {
        return Some(s.unescaped().to_vec());
    }
    if let Some(d) = arg.as_interpolated_string_node() {
        let mut out = Vec::new();
        for part in &d.parts() {
            out.extend_from_slice(part.as_string_node()?.unescaped());
        }
        return Some(out);
    }
    None
}

/// RuboCop's single-argument `add_offense(node, message:) { |c|
/// c.replace(node, replacement) }`.
fn register_single(ctx: &mut Context<'_>, node: &Node<'_>, arg: &Node<'_>, method_name: &[u8]) {
    let replacement = escape_control_chars(ctx.text(arg.span()));
    let msg = message(method_name, &replacement);
    let span = node.span();
    ctx.report_with_fix(
        &RedundantFormat::META,
        span,
        msg,
        Fix { applicability: Applicability::Unsafe, edits: vec![Edit::replace(span, replacement)] },
    );
}

/// RuboCop's `detect_unnecessary_fields`.
fn detect_unnecessary_fields(
    call: &CallNode<'_>,
    node: &Node<'_>,
    ctx: &mut Context<'_>,
    name: &[u8],
) {
    let Some(args_node) = call.arguments() else { return };
    let all_args: Vec<Node<'_>> = args_node.arguments().iter().collect();
    let Some(first) = all_args.first() else { return };
    let Some(str_node) = first.as_string_node() else { return };
    if ext::is_heredoc(first) {
        return;
    }
    let rest = &all_args[1..];
    if rest.is_empty() {
        return;
    }
    if splatted_arguments(call) {
        return;
    }

    let string = str_node.unescaped();
    let Ok(s) = std::str::from_utf8(string) else { return };
    let sequences = format_sequences(s);
    if sequences.is_empty() {
        return;
    }

    let (positional, hash) = split_hash(rest);
    if !all_fields_literal(&sequences, &positional, hash.as_ref(), ctx) {
        return;
    }
    let Some(formatted) = render_format(s, &sequences, &positional, hash.as_ref(), ctx) else {
        return;
    };

    let replacement = quote(&formatted, first, node, ctx);
    let msg = message(name, &replacement);
    let span = node.span();
    ctx.report_with_fix(
        &RedundantFormat::META,
        span,
        msg,
        Fix { applicability: Applicability::Unsafe, edits: vec![Edit::replace(span, replacement)] },
    );
}

/// RuboCop's `splatted_arguments?`.
fn splatted_arguments(call: &CallNode<'_>) -> bool {
    let Some(args) = call.arguments() else { return false };
    args.arguments().iter().any(|a| {
        a.kind() == NodeKind::SplatNode
            || a.as_hash_node()
                .is_some_and(|h| h.elements().iter().any(|e| e.kind() == NodeKind::AssocSplatNode))
            || a.as_keyword_hash_node()
                .is_some_and(|h| h.elements().iter().any(|e| e.kind() == NodeKind::AssocSplatNode))
    })
}

/// Splits `args` into its plain positional elements and a trailing
/// hash/keyword-hash argument, if any -- `arguments.detect(&:hash_type?)`.
fn split_hash<'pr>(args: &[Node<'pr>]) -> (Vec<Node<'pr>>, Option<Node<'pr>>) {
    let mut positional = Vec::new();
    let mut hash = None;
    for arg in args {
        if hash.is_none() && is_hash_type(arg) {
            hash = Some(*arg);
        } else {
            positional.push(*arg);
        }
    }
    (positional, hash)
}

fn is_hash_type(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::HashNode | NodeKind::KeywordHashNode)
}

// ---------------------------------------------------------------------
// Format-sequence parsing
// ---------------------------------------------------------------------

/// One `%`-directive match.
#[derive(Debug, Clone)]
struct FormatSequence {
    percent: bool,
    /// Name text plus whether it was `<name>` (`false`) or `{name}`
    /// (`true`, a template reference, which has no `type`).
    name: Option<(String, bool)>,
    width: Option<Width>,
    /// The `prec`/`prec2` capture's own text (including the leading `.`),
    /// if present -- re-read by [`precision_value`] during rendering.
    precision_text: Option<String>,
    /// The `flags` capture's own text, for [`Flags::parse`].
    flags_text: String,
    /// Explicit `N$` positional reference for the *value* itself (from the
    /// flags run), not the width/precision's own possible `N$`.
    arg_number: Option<u32>,
    type_char: Option<char>,
}

#[derive(Debug, Clone)]
enum Width {
    Fixed(i64),
    /// `*` or `*N$`.
    Star(Option<u32>),
}

impl FormatSequence {
    fn is_template(&self) -> bool {
        self.name.as_ref().is_some_and(|(_, curly)| *curly)
    }
}

fn sequence_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        let flag = r"(?:[ #0+-]|\d+\$)";
        let number = r"(?:\d+|\*(?:\d+\$)?)";
        let precision = r"(?:\.(?:\d+|\*(?:\d+\$)?)?)";
        let ty = "[bBdiouxXeEfgGaAcps]";
        let pattern = format!(
            "%(?P<pct>%)\
             |%(?P<flags>(?:{flag})*)(?:<(?P<name>\\w+)>)?(?P<width>{number})?(?P<prec>{precision})?(?P<type>{ty})\
             |%(?P<width2>{number})?(?P<prec2>{precision})?\\{{(?P<name2>\\w+)\\}}"
        );
        Regex::new(&pattern).expect("valid format-sequence regex")
    });
    &RE
}

/// A trailing `(\d+)\$` inside `text` (upstream's `DIGIT_DOLLAR`).
fn trailing_digit_dollar(text: &str) -> Option<u32> {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)\$").expect("valid regex"));
    let digits = RE.captures(text)?.get(1)?.as_str();
    Some(digits.parse().unwrap_or(u32::MAX))
}

fn parse_width(text: &str) -> Width {
    if let Some(rest) = text.strip_prefix('*') {
        Width::Star(if rest.is_empty() { None } else { trailing_digit_dollar(rest) })
    } else {
        Width::Fixed(text.parse().unwrap_or(0))
    }
}

/// RuboCop's `FormatString#parse`.
fn format_sequences(text: &str) -> Vec<FormatSequence> {
    let re = sequence_regex();
    re.captures_iter(text)
        .map(|caps| {
            if caps.name("pct").is_some() {
                return FormatSequence {
                    percent: true,
                    name: None,
                    width: None,
                    precision_text: None,
                    flags_text: String::new(),
                    arg_number: None,
                    type_char: None,
                };
            }
            if let Some(t) = caps.name("type") {
                let flags_text =
                    caps.name("flags").map_or(String::new(), |m| m.as_str().to_string());
                FormatSequence {
                    percent: false,
                    name: caps.name("name").map(|m| (m.as_str().to_string(), false)),
                    width: caps.name("width").map(|m| parse_width(m.as_str())),
                    precision_text: caps.name("prec").map(|m| m.as_str().to_string()),
                    arg_number: trailing_digit_dollar(&flags_text),
                    flags_text,
                    type_char: t.as_str().chars().next(),
                }
            } else {
                FormatSequence {
                    percent: false,
                    name: caps.name("name2").map(|m| (m.as_str().to_string(), true)),
                    width: caps.name("width2").map(|m| parse_width(m.as_str())),
                    precision_text: caps.name("prec2").map(|m| m.as_str().to_string()),
                    flags_text: String::new(),
                    arg_number: None,
                    type_char: None,
                }
            }
        })
        .collect()
}

// ---------------------------------------------------------------------
// Literal argument values
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
enum LitValue {
    Str(Vec<u8>),
    Int(i64),
    Float(f64),
    /// `numerator / denominator`, from a literal `int / Nr` division.
    Rational(i64, i64),
    /// `(real, imaginary)`, from a bare `Ni` literal or `int + Ni`.
    Complex(i64, i64),
}

fn unwrap_parens<'pr>(node: &Node<'pr>) -> Node<'pr> {
    let mut current = *node;
    while let Some(p) = current.as_parentheses_node() {
        let Some(body) = p.body() else { break };
        let Some(stmts) = body.as_statements_node() else { break };
        let mut iter = stmts.body().iter();
        let Some(first) = iter.next() else { break };
        if iter.next().is_some() {
            break;
        }
        current = first;
    }
    current
}

fn parse_int_bytes(text: &[u8]) -> Option<i64> {
    let s = std::str::from_utf8(text).ok()?.replace('_', "");
    s.parse().ok()
}

fn parse_float_bytes(text: &[u8]) -> Option<f64> {
    let s = std::str::from_utf8(text).ok()?.replace('_', "");
    s.parse().ok()
}

/// `(send int :/ rational)`, possibly inside a `begin`: `3/8r`.
fn literal_rational(node: &Node<'_>, ctx: &Context<'_>) -> Option<LitValue> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"/" {
        return None;
    }
    let receiver = call.receiver()?;
    receiver.as_integer_node()?;
    let numerator = parse_int_bytes(ctx.text(receiver.span()))?;
    let args = call.arguments()?;
    let mut iter = args.arguments().iter();
    let arg = iter.next()?;
    if iter.next().is_some() {
        return None;
    }
    arg.as_rational_node()?;
    let text = ctx.text(arg.span());
    let denom_text = text.strip_suffix(b"r").unwrap_or(text);
    let denominator = parse_int_bytes(denom_text)?;
    Some(LitValue::Rational(numerator, denominator))
}

/// A bare rational literal `8r` (denominator 1).
fn bare_rational(node: &Node<'_>, ctx: &Context<'_>) -> Option<LitValue> {
    node.as_rational_node()?;
    let text = ctx.text(node.span());
    let text = text.strip_suffix(b"r").unwrap_or(text);
    parse_int_bytes(text).map(|n| LitValue::Rational(n, 1))
}

/// `Ni` or `(send int :+ Ni)`: `1i`, `5+0i`.
fn literal_complex(node: &Node<'_>, ctx: &Context<'_>) -> Option<LitValue> {
    if node.as_imaginary_node().is_some() {
        let text = ctx.text(node.span());
        let text = text.strip_suffix(b"i").unwrap_or(text);
        return parse_int_bytes(text).map(|im| LitValue::Complex(0, im));
    }
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"+" {
        return None;
    }
    let receiver = call.receiver()?;
    receiver.as_integer_node()?;
    let real = parse_int_bytes(ctx.text(receiver.span()))?;
    let args = call.arguments()?;
    let mut iter = args.arguments().iter();
    let arg = iter.next()?;
    if iter.next().is_some() {
        return None;
    }
    arg.as_imaginary_node()?;
    let text = ctx.text(arg.span());
    let text = text.strip_suffix(b"i").unwrap_or(text);
    let imag = parse_int_bytes(text)?;
    Some(LitValue::Complex(real, imag))
}

fn dstr_value(node: &Node<'_>, ctx: &Context<'_>) -> Option<Vec<u8>> {
    let d = node.as_interpolated_string_node()?;
    let mut out = Vec::new();
    for part in &d.parts() {
        if let Some(s) = part.as_string_node() {
            out.extend_from_slice(s.unescaped());
        } else {
            out.extend_from_slice(ctx.text(part.span()));
        }
    }
    Some(out)
}

fn dsym_value(node: &Node<'_>, ctx: &Context<'_>) -> Option<Vec<u8>> {
    let d = node.as_interpolated_symbol_node()?;
    let mut out = Vec::new();
    for part in &d.parts() {
        if let Some(s) = part.as_string_node() {
            out.extend_from_slice(s.unescaped());
        } else {
            out.extend_from_slice(ctx.text(part.span()));
        }
    }
    Some(out)
}

/// `argument_value`/`typed_argument_value`.
fn literal_value(node: &Node<'_>, ctx: &Context<'_>) -> Option<LitValue> {
    let node = unwrap_parens(node);
    if node.kind() == NodeKind::NilNode {
        return Some(LitValue::Str(Vec::new()));
    }
    if let Some(r) = literal_rational(&node, ctx).or_else(|| bare_rational(&node, ctx)) {
        return Some(r);
    }
    if let Some(c) = literal_complex(&node, ctx) {
        return Some(c);
    }
    if let Some(s) = node.as_string_node() {
        return Some(LitValue::Str(s.unescaped().to_vec()));
    }
    if let Some(v) = dstr_value(&node, ctx) {
        return Some(LitValue::Str(v));
    }
    if let Some(s) = node.as_symbol_node() {
        return Some(LitValue::Str(s.unescaped().to_vec()));
    }
    if let Some(v) = dsym_value(&node, ctx) {
        return Some(LitValue::Str(v));
    }
    if node.as_true_node().is_some() {
        return Some(LitValue::Str(b"true".to_vec()));
    }
    if node.as_false_node().is_some() {
        return Some(LitValue::Str(b"false".to_vec()));
    }
    if let Some(f) = node.as_float_node() {
        return Some(LitValue::Float(f.value()));
    }
    if node.as_integer_node().is_some() {
        return parse_int_bytes(ctx.text(node.span())).map(LitValue::Int);
    }
    None
}

/// Whether `node` is one of `ACCEPTABLE_LITERAL_TYPES`.
fn is_acceptable_literal(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let unwrapped = unwrap_parens(node);
    matches!(
        unwrapped.kind(),
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
    ) || literal_rational(&unwrapped, ctx).is_some()
        || bare_rational(&unwrapped, ctx).is_some()
        || literal_complex(&unwrapped, ctx).is_some()
}

/// Ruby's `Integer(float_value)` truncation, without a bare `as` cast:
/// out-of-`i64`-range floats saturate rather than wrapping.
fn f64_trunc_to_i64(f: f64) -> Option<i64> {
    let t = f.trunc();
    if !t.is_finite() {
        return None;
    }
    if t >= 9_223_372_036_854_775_807.0 {
        return Some(i64::MAX);
    }
    if t <= -9_223_372_036_854_775_808.0 {
        return Some(i64::MIN);
    }
    format!("{t:.0}").parse().ok()
}

fn as_i64(value: &LitValue) -> Option<i64> {
    match value {
        LitValue::Int(n) => Some(*n),
        LitValue::Float(f) => f64_trunc_to_i64(*f),
        LitValue::Str(s) => parse_int_bytes(s),
        LitValue::Rational(n, d) if *d != 0 => Some(n / d),
        LitValue::Complex(re, _) => Some(*re),
        LitValue::Rational(..) => None,
    }
}

/// `i64` to `f64`, without a bare `as` cast (every value this port ever
/// builds comes from parsing a literal's own decimal source text, so a
/// string round-trip is simplest).
fn i64_to_f64(n: i64) -> f64 {
    format!("{n}").parse().unwrap_or(0.0)
}

fn as_f64(value: &LitValue) -> Option<f64> {
    match value {
        LitValue::Int(n) => Some(i64_to_f64(*n)),
        LitValue::Float(f) => Some(*f),
        LitValue::Str(s) => parse_float_bytes(s),
        LitValue::Rational(n, d) if *d != 0 => Some(i64_to_f64(*n) / i64_to_f64(*d)),
        LitValue::Complex(re, _) => Some(i64_to_f64(*re)),
        LitValue::Rational(..) => None,
    }
}

fn stringify(value: &LitValue) -> Vec<u8> {
    match value {
        LitValue::Str(s) => s.clone(),
        LitValue::Int(n) => n.to_string().into_bytes(),
        LitValue::Float(f) => ruby_float_to_s(*f).into_bytes(),
        LitValue::Rational(n, d) => format!("{n}/{d}").into_bytes(),
        LitValue::Complex(re, im) => {
            if *im < 0 {
                format!("{re}-{}i", -im).into_bytes()
            } else {
                format!("{re}+{im}i").into_bytes()
            }
        }
    }
}

/// Ruby's `Float#to_s`: always at least one digit after the point.
fn ruby_float_to_s(f: f64) -> String {
    if f.fract() == 0.0 && f.is_finite() {
        format!("{f:.1}")
    } else {
        format!("{f}")
    }
}

/// `integer?`: numeric-or-numeric-string, parseable as an integer.
fn is_integer_like(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    match literal_value(node, ctx) {
        Some(LitValue::Int(_) | LitValue::Complex(..) | LitValue::Float(_)) => true,
        Some(LitValue::Str(s)) => parse_int_bytes(&s).is_some(),
        Some(LitValue::Rational(_, d)) => d != 0,
        None => false,
    }
}

/// `float?`: numeric-or-numeric-string, parseable as a float.
fn is_float_like(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    literal_value(node, ctx).as_ref().and_then(as_f64).is_some()
}

// ---------------------------------------------------------------------
// Validation (`all_fields_literal?`)
// ---------------------------------------------------------------------

fn positional_argument<'pr>(args: &[Node<'pr>], n: u32) -> Option<Node<'pr>> {
    args.get(usize::try_from(n.checked_sub(1)?).unwrap_or(usize::MAX)).copied()
}

fn find_hash_value<'pr>(hash: &Node<'pr>, name: &str) -> Option<Node<'pr>> {
    let elements: Vec<Node<'pr>> = if let Some(h) = hash.as_hash_node() {
        h.elements().iter().collect()
    } else {
        hash.as_keyword_hash_node()?.elements().iter().collect()
    };
    for el in elements {
        let assoc = el.as_assoc_node()?;
        let key_matches =
            assoc.key().as_symbol_node().is_some_and(|s| s.unescaped() == name.as_bytes());
        if key_matches {
            return Some(assoc.value());
        }
    }
    None
}

fn unknown_variable_width(width: Option<&Width>, args: &[Node<'_>], ctx: &Context<'_>) -> bool {
    let Some(Width::Star(explicit)) = width else { return false };
    let Some(arg) = (match explicit {
        Some(n) => positional_argument(args, *n),
        None => args.first().copied(),
    }) else {
        return true;
    };
    !is_integer_like(&arg, ctx) && !is_float_like(&arg, ctx)
}

fn find_argument<'pr>(
    seq: &FormatSequence,
    args: &mut Vec<Node<'pr>>,
    hash: Option<&Node<'pr>>,
) -> Option<Node<'pr>> {
    if let Some((name, _)) = &seq.name {
        return find_hash_value(hash?, name);
    }
    if let Some(Width::Star(explicit)) = &seq.width {
        match explicit {
            Some(n) => {
                if let Some(idx) = n.checked_sub(1).and_then(|i| usize::try_from(i).ok()) {
                    if idx < args.len() {
                        args.remove(idx);
                    }
                }
            }
            None => {
                if !args.is_empty() {
                    args.remove(0);
                }
            }
        }
        return if args.is_empty() { None } else { Some(args.remove(0)) };
    }
    if let Some(n) = seq.arg_number {
        return positional_argument(args, n);
    }
    if args.is_empty() {
        None
    } else {
        Some(args.remove(0))
    }
}

fn matching_argument(seq: &FormatSequence, node: &Node<'_>, ctx: &Context<'_>) -> bool {
    if seq.is_template() {
        return is_acceptable_literal(node, ctx);
    }
    match seq.type_char {
        Some('s') => is_acceptable_literal(node, ctx),
        Some('d' | 'i' | 'u') => is_integer_like(node, ctx),
        Some('f') => is_float_like(node, ctx),
        _ => false,
    }
}

/// RuboCop's `all_fields_literal?`. A percent (`%%`) sequence always fails
/// upstream's own `sequences.size == count` check (its `next` never
/// increments `count`), so this short-circuits on the first one instead of
/// replicating the counter.
fn all_fields_literal(
    sequences: &[FormatSequence],
    positional: &[Node<'_>],
    hash: Option<&Node<'_>>,
    ctx: &Context<'_>,
) -> bool {
    if sequences.is_empty() {
        return false;
    }
    let mut args: Vec<Node<'_>> = positional.to_vec();
    for seq in sequences {
        if seq.percent {
            return false;
        }
        if unknown_variable_width(seq.width.as_ref(), &args, ctx) {
            return false;
        }
        let Some(argument) = find_argument(seq, &mut args, hash) else { return false };
        if !matching_argument(seq, &argument, ctx) {
            return false;
        }
        if (seq.width.is_some() || seq.precision_text.is_some())
            && argument.as_interpolated_string_node().is_some()
        {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------
// Rendering (replicates Ruby's own `format` for the directives
// `all_fields_literal?` accepts)
// ---------------------------------------------------------------------

struct Cursor<'a, 'pr> {
    args: &'a [Node<'pr>],
    idx: usize,
}

impl<'pr> Cursor<'_, 'pr> {
    fn next(&mut self) -> Option<Node<'pr>> {
        let v = self.args.get(self.idx).copied();
        self.idx += 1;
        v
    }
}

/// `width`'s resolved runtime value: `Absent` when the sequence had none,
/// `Value` when it did (fixed or `*`-consumed). Kept as its own enum rather
/// than `Option<Option<i64>>`.
enum Resolved {
    Absent,
    Value(i64),
}

fn resolve_count(
    width: Option<&Width>,
    cursor: &mut Cursor<'_, '_>,
    ctx: &Context<'_>,
) -> Option<Resolved> {
    match width {
        None => Some(Resolved::Absent),
        Some(Width::Fixed(n)) => Some(Resolved::Value(*n)),
        Some(Width::Star(explicit)) => {
            let node = match explicit {
                Some(n) => positional_argument(cursor.args, *n)?,
                None => cursor.next()?,
            };
            let value = literal_value(&node, ctx)?;
            Some(Resolved::Value(as_i64(&value)?))
        }
    }
}

/// RuboCop's `register_all_fields_literal`'s `format(string,
/// *format_arguments)`, restricted to the directives validation accepts.
fn render_format(
    fmt: &str,
    sequences: &[FormatSequence],
    positional: &[Node<'_>],
    hash: Option<&Node<'_>>,
    ctx: &Context<'_>,
) -> Option<Vec<u8>> {
    let re = sequence_regex();
    let mut cursor = Cursor { args: positional, idx: 0 };
    let mut out = Vec::new();
    let mut last_end = 0;
    let fmt_bytes = fmt.as_bytes();
    for (seq, m) in sequences.iter().zip(re.find_iter(fmt)) {
        out.extend_from_slice(&fmt_bytes[last_end..m.start()]);
        last_end = m.end();
        if seq.percent {
            out.push(b'%');
            continue;
        }

        let width = match resolve_count(seq.width.as_ref(), &mut cursor, ctx)? {
            Resolved::Absent => None,
            Resolved::Value(v) => Some(v),
        };
        let precision = match &seq.precision_text {
            Some(text) => Some(precision_value(text, &mut cursor, ctx)?),
            None => None,
        };

        let value_node = if let Some((name, _)) = &seq.name {
            find_hash_value(hash?, name)?
        } else if let Some(n) = seq.arg_number {
            positional_argument(cursor.args, n)?
        } else {
            cursor.next()?
        };
        let value = literal_value(&value_node, ctx)?;

        let rendered = if seq.is_template() {
            render_s(&seq.flags_text, width, precision, &stringify(&value))
        } else {
            match seq.type_char {
                Some('s') => render_s(&seq.flags_text, width, precision, &stringify(&value)),
                Some('d' | 'i' | 'u') => {
                    render_int(&seq.flags_text, width, precision, as_i64(&value)?)
                }
                Some('f') => render_float(&seq.flags_text, width, precision, as_f64(&value)?),
                _ => return None,
            }
        };
        out.extend_from_slice(&rendered);
    }
    out.extend_from_slice(&fmt_bytes[last_end..]);
    Some(out)
}

/// RuboCop's precision handling: `precision_text` is the `prec`/`prec2`
/// capture's own text, e.g. `".2"`, `".*"`, `".*2$"`, or just `"."`.
fn precision_value(
    precision_text: &str,
    cursor: &mut Cursor<'_, '_>,
    ctx: &Context<'_>,
) -> Option<i64> {
    let rest = precision_text.strip_prefix('.')?;
    if let Some(after_star) = rest.strip_prefix('*') {
        let explicit = trailing_digit_dollar(after_star);
        let node = match explicit {
            Some(n) => positional_argument(cursor.args, n)?,
            None => cursor.next()?,
        };
        let value = literal_value(&node, ctx)?;
        return as_i64(&value);
    }
    if rest.is_empty() {
        return Some(0);
    }
    rest.parse().ok()
}

fn flag_minus(flags_text: &str) -> bool {
    flags_text.contains('-')
}

fn flag_zero(flags_text: &str) -> bool {
    flags_text.contains('0')
}

fn flag_plus(flags_text: &str) -> bool {
    flags_text.contains('+')
}

fn flag_space(flags_text: &str) -> bool {
    flags_text.contains(' ')
}

/// A non-negative byte count, clamped to `usize::MAX` rather than wrapping.
fn as_count(n: i64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

fn pad(body: Vec<u8>, width: Option<i64>, minus: bool, zero: bool, sign_len: usize) -> Vec<u8> {
    let Some(w) = width else { return body };
    let (w, minus) = if w < 0 { (-w, true) } else { (w, minus) };
    let w = as_count(w);
    if body.len() >= w {
        return body;
    }
    let pad_len = w - body.len();
    if minus {
        let mut out = body;
        out.extend(std::iter::repeat_n(b' ', pad_len));
        out
    } else if zero {
        let mut out = body[..sign_len].to_vec();
        out.extend(std::iter::repeat_n(b'0', pad_len));
        out.extend_from_slice(&body[sign_len..]);
        out
    } else {
        let mut out = Vec::with_capacity(w);
        out.extend(std::iter::repeat_n(b' ', pad_len));
        out.extend_from_slice(&body);
        out
    }
}

fn render_s(flags_text: &str, width: Option<i64>, precision: Option<i64>, value: &[u8]) -> Vec<u8> {
    let text = if let Some(p) = precision {
        let p = as_count(p.max(0));
        let s = String::from_utf8_lossy(value);
        s.chars().take(p).collect::<String>().into_bytes()
    } else {
        value.to_vec()
    };
    pad(text, width, flag_minus(flags_text), false, 0)
}

fn render_int(flags_text: &str, width: Option<i64>, precision: Option<i64>, n: i64) -> Vec<u8> {
    let negative = n < 0;
    let digits = n.unsigned_abs().to_string();
    let digits = if let Some(p) = precision {
        let p = as_count(p.max(0));
        if n == 0 && p == 0 {
            String::new()
        } else {
            format!("{digits:0>p$}")
        }
    } else {
        digits
    };
    let sign: &[u8] = if negative {
        b"-"
    } else if flag_plus(flags_text) {
        b"+"
    } else if flag_space(flags_text) {
        b" "
    } else {
        b""
    };
    let mut body = sign.to_vec();
    body.extend_from_slice(digits.as_bytes());
    let zero_ok = flag_zero(flags_text) && precision.is_none();
    pad(body, width, flag_minus(flags_text), zero_ok, sign.len())
}

fn render_float(flags_text: &str, width: Option<i64>, precision: Option<i64>, f: f64) -> Vec<u8> {
    let p = as_count(precision.unwrap_or(6).max(0));
    let negative = f.is_sign_negative();
    let digits = format!("{:.*}", p, f.abs());
    let sign: &[u8] = if negative {
        b"-"
    } else if flag_plus(flags_text) {
        b"+"
    } else if flag_space(flags_text) {
        b" "
    } else {
        b""
    };
    let mut body = sign.to_vec();
    body.extend_from_slice(digits.as_bytes());
    pad(body, width, flag_minus(flags_text), flag_zero(flags_text), sign.len())
}

// ---------------------------------------------------------------------
// Quoting and escaping
// ---------------------------------------------------------------------

/// RuboCop's `quote`.
fn quote(
    formatted: &[u8],
    first_arg: &Node<'_>,
    call_node: &Node<'_>,
    ctx: &Context<'_>,
) -> Vec<u8> {
    let Some(str_node) = first_arg.as_string_node() else { return escape_control_chars(formatted) };
    let (Some(open_loc), Some(close_loc)) = (str_node.opening_loc(), str_node.closing_loc()) else {
        return escape_control_chars(formatted);
    };
    let mut start_delim = ctx.text(open_loc.span()).to_vec();
    let end_delim = ctx.text(close_loc.span()).to_vec();

    if has_dstr_or_dsym_descendant(call_node) {
        if start_delim == b"'" {
            start_delim = b"\"".to_vec();
            let mut out = start_delim;
            out.extend_from_slice(&escape_control_chars(formatted));
            out.extend_from_slice(b"\"");
            return out;
        }
        if let Some(rest) = start_delim.strip_prefix(b"%q") {
            if rest.len() == 1 {
                let mut new_start = b"%Q".to_vec();
                new_start.push(rest[0]);
                let mut out = new_start;
                out.extend_from_slice(&escape_control_chars(formatted));
                out.extend_from_slice(&end_delim);
                return out;
            }
        }
    }

    let mut out = start_delim;
    out.extend_from_slice(&escape_control_chars(formatted));
    out.extend_from_slice(&end_delim);
    out
}

fn has_dstr_or_dsym_descendant(node: &Node<'_>) -> bool {
    let mut found = false;
    ruby_ast::each_descendant(node, &mut |n: &Node<'_>| {
        if matches!(n.kind(), NodeKind::InterpolatedStringNode | NodeKind::InterpolatedSymbolNode) {
            found = true;
        }
    });
    found
}

/// RuboCop's `escape_control_chars`: `gsub(/\p{Cc}/) { |s| s.dump[1..-2] }`.
fn escape_control_chars(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    for &b in text {
        match b {
            0x00 => out.extend_from_slice(b"\\0"),
            0x07 => out.extend_from_slice(b"\\a"),
            0x08 => out.extend_from_slice(b"\\b"),
            0x09 => out.extend_from_slice(b"\\t"),
            0x0A => out.extend_from_slice(b"\\n"),
            0x0B => out.extend_from_slice(b"\\v"),
            0x0C => out.extend_from_slice(b"\\f"),
            0x0D => out.extend_from_slice(b"\\r"),
            0x1B => out.extend_from_slice(b"\\e"),
            0x01..=0x1F | 0x7F..=0x9F => out.extend_from_slice(format!("\\x{b:02X}").as_bytes()),
            _ => out.push(b),
        }
    }
    out
}
