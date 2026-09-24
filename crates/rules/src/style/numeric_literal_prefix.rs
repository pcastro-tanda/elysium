//! `Style/NumericLiteralPrefix`, ported from RuboCop's
//! `lib/rubocop/cop/style/numeric_literal_prefix.rb` plus the `IntegerNode`
//! mixin (`lib/rubocop/cop/mixin/integer_node.rb`) it includes.
//!
//! RuboCop's own parser fuses a leading unary `-`/`+` sign directly onto an
//! adjacent integer literal's `source`/node range, so its `integer_part`
//! helper has to strip a leading sign character before pattern-matching the
//! digit run. Prism never does this fusion: a signed literal like `-0X1AC`
//! parses as a `CallNode` named `-@` whose receiver is the plain `IntegerNode`
//! `0X1AC`, so [`NodeKind::IntegerNode`]'s own `source` already excludes any
//! sign and no stripping is needed here.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `OCTAL_ZERO_ONLY_MSG`.
const OCTAL_ZERO_ONLY_MSG: &str = "Use 0 for octal literals.";
/// RuboCop's `OCTAL_MSG`.
const OCTAL_MSG: &str = "Use 0o for octal literals.";
/// RuboCop's `HEX_MSG`.
const HEX_MSG: &str = "Use 0x for hexadecimal literals.";
/// RuboCop's `BINARY_MSG`.
const BINARY_MSG: &str = "Use 0b for binary literals.";
/// RuboCop's `DECIMAL_MSG`.
const DECIMAL_MSG: &str = "Do not use prefixes for decimal literals.";

/// RuboCop's `literal_type`/`octal_literal_type`/`hex_bin_dec_literal_type`
/// result: which prefix family a literal's source matches, and (for octal)
/// which of the two mutually exclusive styles it was matched under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LiteralType {
    OctalZeroOnly,
    Octal,
    Hex,
    Binary,
    Decimal,
}

impl LiteralType {
    /// RuboCop's `message`: `self.class.const_get(:"#{literal_type(node).upcase}_MSG")`.
    const fn message(self) -> &'static str {
        match self {
            Self::OctalZeroOnly => OCTAL_ZERO_ONLY_MSG,
            Self::Octal => OCTAL_MSG,
            Self::Hex => HEX_MSG,
            Self::Binary => BINARY_MSG,
            Self::Decimal => DECIMAL_MSG,
        }
    }

    /// RuboCop's `format_<type>`: rewrites `source` (the literal's own text,
    /// already known to match this variant's regex) to the corrected form.
    fn format(self, source: &str) -> String {
        match self {
            // RuboCop's `source.sub(/^0[Oo]?/, '0')`: `source` matched
            // `OCTAL_ZERO_ONLY_REGEX` (`^0[Oo][0-7]+$`), so the prefix is
            // always exactly two bytes (`0O` or `0o`).
            Self::OctalZeroOnly => format!("0{}", &source[2..]),
            // RuboCop's `source.sub(/^0O?/, '0o')`: `source` matched
            // `OCTAL_REGEX` (`^0O?[0-7]+$`), so the prefix is either `0` or
            // `0O` (never lowercase `o`).
            Self::Octal => match source.strip_prefix("0O") {
                Some(rest) => format!("0o{rest}"),
                None => format!("0o{}", &source[1..]),
            },
            // RuboCop's `source.sub(/^0X/, '0x')`: the prefix is always `0X`.
            Self::Hex => format!("0x{}", &source[2..]),
            // RuboCop's `source.sub(/^0B/, '0b')`: the prefix is always `0B`.
            Self::Binary => format!("0b{}", &source[2..]),
            // RuboCop's `source.sub(/^0[dD]/, '')`: the prefix is always `0d`
            // or `0D`, dropped entirely (decimals get no prefix at all).
            Self::Decimal => source[2..].to_string(),
        }
    }
}

/// RuboCop's `OCTAL_ZERO_ONLY_REGEX = /^0[Oo][0-7]+$/`.
fn matches_octal_zero_only(bytes: &[u8]) -> bool {
    bytes.len() >= 3
        && bytes[0] == b'0'
        && matches!(bytes[1], b'O' | b'o')
        && bytes[2..].iter().all(|&b| (b'0'..=b'7').contains(&b))
}

/// RuboCop's `OCTAL_REGEX = /^0O?[0-7]+$/` (the optional prefix letter is
/// uppercase-only; a lowercase `o` therefore falls through to the digit
/// check and fails since `o` is not an octal digit).
fn matches_octal(bytes: &[u8]) -> bool {
    if bytes.first() != Some(&b'0') {
        return false;
    }
    let rest = if bytes.get(1) == Some(&b'O') { &bytes[2..] } else { &bytes[1..] };
    !rest.is_empty() && rest.iter().all(|&b| (b'0'..=b'7').contains(&b))
}

/// RuboCop's `HEX_REGEX = /^0X[0-9A-F]+$/` (uppercase prefix and digits
/// only; a literal with any lowercase hex digit does not match).
fn matches_hex(bytes: &[u8]) -> bool {
    bytes.len() >= 3
        && bytes[0] == b'0'
        && bytes[1] == b'X'
        && bytes[2..].iter().all(|&b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
}

/// RuboCop's `BINARY_REGEX = /^0B[01]+$/`.
fn matches_binary(bytes: &[u8]) -> bool {
    bytes.len() >= 3
        && bytes[0] == b'0'
        && bytes[1] == b'B'
        && bytes[2..].iter().all(|&b| b == b'0' || b == b'1')
}

/// RuboCop's `DECIMAL_REGEX = /^0[dD][0-9]+$/`.
fn matches_decimal(bytes: &[u8]) -> bool {
    bytes.len() >= 3
        && bytes[0] == b'0'
        && matches!(bytes[1], b'd' | b'D')
        && bytes[2..].iter().all(u8::is_ascii_digit)
}

/// RuboCop's `literal_type`: `octal_literal_type(literal) ||
/// hex_bin_dec_literal_type(literal)`.
fn literal_type(source: &str, zero_only: bool) -> Option<LiteralType> {
    let bytes = source.as_bytes();
    if matches_octal_zero_only(bytes) && zero_only {
        return Some(LiteralType::OctalZeroOnly);
    }
    if matches_octal(bytes) && !zero_only {
        return Some(LiteralType::Octal);
    }
    if matches_hex(bytes) {
        return Some(LiteralType::Hex);
    }
    if matches_binary(bytes) {
        return Some(LiteralType::Binary);
    }
    if matches_decimal(bytes) {
        return Some(LiteralType::Decimal);
    }
    None
}

/// Checks for octal, hex, binary, and decimal literals using uppercase
/// prefixes and corrects them to a lowercase prefix or no prefix (in the
/// case of decimals).
#[derive(Debug, Clone)]
pub struct NumericLiteralPrefix {
    /// RuboCop's `octal_zero_only?`: `EnforcedOctalStyle == 'zero_only'`.
    zero_only: bool,
}

impl Rule for NumericLiteralPrefix {
    const META: RuleMeta = RuleMeta {
        name: "Style/NumericLiteralPrefix",
        department: Department::Style,
        summary: "Use smallcase prefixes for numeric literals.",
        explanation: "\
```ruby
# EnforcedOctalStyle: zero_with_o (default)

# bad - missing octal prefix
num = 01234

# bad - uppercase prefix
num = 0O1234
num = 0X12AB
num = 0B10101

# bad - redundant decimal prefix
num = 0D1234
num = 0d1234

# good
num = 0o1234
num = 0x12AB
num = 0b10101
num = 1234
```

```ruby
# EnforcedOctalStyle: zero_only

# bad
num = 0o1234
num = 0O1234

# good
num = 01234
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IntegerNode],
        config: &[ConfigOption {
            name: "EnforcedOctalStyle",
            default: ConfigDefault::Str("zero_with_o"),
            allowed: &["zero_with_o", "zero_only"],
            doc: "Whether octal literals must use the `0o` prefix or a bare `0`.",
        }],
        blind_spots: "\
`HEX_REGEX`/`BINARY_REGEX` are ported byte-for-byte from RuboCop's own
case-sensitive regexes: a literal like `0X1ac` (uppercase `X` prefix but a
lowercase hex digit) or `0xABC` (lowercase `x` prefix, uppercase digits, thus
already 'correct' and left alone) is not touched by either RuboCop or this
port, since the whole digit run's case has to agree with the regex before it
is considered a match at all; this is upstream's own limitation, not a gap
introduced here. Because Prism never fuses a leading unary `-`/`+` sign onto
an `IntegerNode` (unlike RuboCop's own parser), a signed literal such as
`-0X1AC` is reported/fixed at the narrower span of the literal alone
(excluding the sign); RuboCop's fix for that exact shape is actually a
no-op bug (its corrector operates on the sign-inclusive node source, which
no longer matches the anchored `^0X` prefix regex), so this port's behavior
is strictly more correct there rather than a regression.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let zero_only = options.style("EnforcedOctalStyle")? == "zero_only";
        Ok(Self { zero_only })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let span = node.span();
        let Ok(source) = std::str::from_utf8(ctx.text(span)) else { return };
        let Some(kind) = literal_type(source, self.zero_only) else { return };

        let fixed = kind.format(source);
        ctx.report_with_fix(
            &Self::META,
            span,
            kind.message(),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, fixed.into_bytes())],
            },
        );
    }
}
