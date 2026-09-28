//! `Style/ExponentialNotation`, ported from RuboCop's
//! `lib/rubocop/cop/style/exponential_notation.rb`.
//!
//! # `node.source['e']`
//!
//! Upstream's `offense?` guards every style check on `node.source['e']`
//! (`String#[]` substring presence), a **lowercase**-only check. Ruby's
//! numeric-literal lexer accepts either `e` or `E` to introduce a float
//! exponent (`1E10` and `1e10` both parse to the same `Float`), but a
//! literal spelled with a bare uppercase `E` and no lowercase `e` anywhere
//! else in its source never matches `source['e']`, so `offense?` returns
//! `false` before ever calling `scientific?`/`engineering?`/`integral?` --
//! an uppercase-only exponent letter is silently never flagged, regardless
//! of style or how skewed its mantissa is. This is an upstream blind spot,
//! reproduced here verbatim by testing for a lowercase `e` byte specifically
//! (not an ASCII-case-insensitive search).
//!
//! # `node.source.split('e')`
//!
//! Every style splits `node.source` on `e` and reads only the first
//! (`mantissa`) or first two (`mantissa`, `exponent`) fields, discarding the
//! rest -- moot here since a legal Ruby float literal can only ever contain
//! one `e`, so a plain `split_once('e')` is exact.
//!
//! # Sign fusion
//!
//! Prism fuses an immediately-adjacent unary `-`/`+` directly into a
//! `FloatNode`'s own span/source (see `Style/NumericLiterals`' module
//! docs for the general rule), matching whitequark's parser closely enough
//! that every signed-mantissa fixture (`-9.999e3`, `-0.09e3`, ...) is
//! covered by [`Node::span`] alone; a sign separated from the literal by
//! whitespace instead parses as a `CallNode` wrapping an unsigned
//! `FloatNode` (which this cop, like upstream's `on_float`, never visits at
//! all), an untested edge case for both RuboCop and this port.

use std::sync::LazyLock;

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Scientific,
    Engineering,
    Integral,
}

/// Enforces consistency when using exponential notation for numbers.
#[derive(Debug, Clone)]
pub struct ExponentialNotation {
    style: Style,
}

/// RuboCop's `scientific?`: `/^-?[1-9](\.\d*[0-9])?$/` against the mantissa.
fn is_scientific(mantissa: &str) -> bool {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^-?[1-9](\.\d*[0-9])?$").expect("valid"));
    RE.is_match(mantissa)
}

/// RuboCop's `integral?`: `/^-?[1-9](\d*[1-9])?$/` against the mantissa.
fn is_integral(mantissa: &str) -> bool {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^-?[1-9](\d*[1-9])?$").expect("valid"));
    RE.is_match(mantissa)
}

/// RuboCop's `engineering?`.
fn is_engineering(mantissa: &str, exponent: &str) -> bool {
    static EXPONENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-?\d+$").expect("valid"));
    static FOUR_DIGITS_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^-?\d{4}").expect("valid"));
    static LEADING_ZERO_DIGIT_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^-?0\d").expect("valid"));
    // Upstream's `/^-?0.0/`: the middle `.` is an unescaped regex
    // metacharacter (matches any single character), not a literal dot.
    static LEADING_ZERO_DOT_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^-?0.0").expect("valid"));

    if !EXPONENT_RE.is_match(exponent) {
        return false;
    }
    let Ok(exponent_value) = exponent.parse::<i64>() else { return false };
    if exponent_value % 3 != 0 {
        return false;
    }
    if FOUR_DIGITS_RE.is_match(mantissa) {
        return false;
    }
    if LEADING_ZERO_DIGIT_RE.is_match(mantissa) {
        return false;
    }
    if LEADING_ZERO_DOT_RE.is_match(mantissa) {
        return false;
    }
    true
}

/// RuboCop's `offense?`: `source['e']` (a lowercase-only substring check --
/// see the module docs) then `!scientific?`/`!engineering?`/`!integral?`
/// per style.
fn is_offense(style: Style, source: &str) -> bool {
    if !source.contains('e') {
        return false;
    }
    let Some((mantissa, exponent)) = source.split_once('e') else { return false };
    match style {
        Style::Scientific => !is_scientific(mantissa),
        Style::Engineering => !is_engineering(mantissa, exponent),
        Style::Integral => !is_integral(mantissa),
    }
}

impl Rule for ExponentialNotation {
    const META: RuleMeta = RuleMeta {
        name: "Style/ExponentialNotation",
        department: Department::Style,
        summary: "Enforces consistency when using exponential notation for numbers in the code.",
        explanation: "\
Enforces consistency when using exponential notation
for numbers in the code (eg `1.2e4`). Different styles are supported:

* `scientific` which enforces a mantissa between 1 (inclusive) and 10 (exclusive).
* `engineering` which enforces the exponent to be a multiple of 3 and the mantissa
  to be between 0.1 (inclusive) and 1000 (exclusive).
* `integral` which enforces the mantissa to always be a whole number without
  trailing zeroes.

```ruby
# EnforcedStyle: scientific (default)
# Enforces a mantissa between 1 (inclusive) and 10 (exclusive).

# bad
10e6
0.3e4
11.7e5
3.14e0

# good
1e7
3e3
1.17e6
3.14
```

```ruby
# EnforcedStyle: engineering
# Enforces using multiple of 3 exponents,
# mantissa should be between 0.1 (inclusive) and 1000 (exclusive)

# bad
3.2e7
0.1e5
12e5
1232e6

# good
32e6
10e3
1.2e6
1.232e9
```

```ruby
# EnforcedStyle: integral
# Enforces the mantissa to have no decimal part and no
# trailing zeroes.

# bad
3.2e7
0.1e5
120e4

# good
32e6
1e4
12e5
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::FloatNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("scientific"),
            allowed: &["scientific", "engineering", "integral"],
            doc: "Which exponential-notation mantissa/exponent convention to enforce.",
        }],
        blind_spots: "\
An exponent letter spelled as a bare uppercase `E` (with no lowercase `e` elsewhere in the
literal's source, e.g. `1E10`) is never flagged, matching upstream's own
`node.source['e']`-guarded (lowercase-only) `offense?` check -- an upstream blind spot, not one
introduced by this port. A sign separated from the literal by whitespace parses as a `CallNode`
wrapping an unsigned `FloatNode`, which -- like upstream's `on_float` -- this cop never visits.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "engineering" => Style::Engineering,
            "integral" => Style::Integral,
            _ => Style::Scientific,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let span = node.span();
        let Ok(source) = std::str::from_utf8(ctx.text(span)) else { return };
        if !is_offense(self.style, source) {
            return;
        }
        let message = match self.style {
            Style::Scientific => "Use a mantissa >= 1 and < 10.",
            Style::Engineering => {
                "Use an exponent divisible by 3 and a mantissa >= 0.1 and < 1000."
            }
            Style::Integral => "Use an integer as mantissa, without trailing zero.",
        };
        ctx.report(&Self::META, span, message);
    }
}
