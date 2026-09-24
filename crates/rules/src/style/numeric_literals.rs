//! `Style/NumericLiterals`, ported from RuboCop's
//! `lib/rubocop/cop/style/numeric_literals.rb` plus the `IntegerNode` and
//! `AllowedPattern` mixins it includes.
//!
//! RuboCop's own parser fuses a unary `-`/`+` sign directly onto an adjacent
//! numeric literal, even across intervening whitespace or newlines (Ruby's
//! lexer only special-cases the *lack* of another token in between, not the
//! lack of whitespace): `-\n  12345` is a single `int` node whose source
//! spans from the sign to the last digit. Prism instead parses that as a
//! `CallNode` named `-@`/`+@` whose receiver is the (unsigned) literal, only
//! collapsing the two into one literal node when the sign is *immediately*
//! adjacent to the digits. [`effective_span`] closes this gap: for a literal
//! that is the sole receiver of such a bare unary-sign call, with nothing
//! but the sign and whitespace between the call's start and the literal's
//! start, it widens the reported/replaced range to the call's own span
//! (which already covers exactly that RuboCop range).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    NodeInfo, OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str =
    "Use underscores(_) as thousands separator and separate every 3 digits with them.";

/// Looks for big numeric literals without `_` between groups of digits.
#[derive(Debug, Clone)]
pub struct NumericLiterals {
    min_digits: usize,
    allowed_numbers: Vec<String>,
    allowed_patterns: Vec<Regex>,
    /// RuboCop's `short_group_regex`: `/_\d{1,2}(_|$)/` under `Strict: true`,
    /// else `/_\d{1,2}_/`.
    strict: bool,
}

impl NumericLiterals {
    /// RuboCop's `check`: decides whether `int` (the digit run, sign
    /// stripped, with everything from the first `e`/`E`/`.` onward cut off --
    /// RuboCop's `integer_part`) is undelimited or misdelimited, skipping
    /// allowed numbers/patterns and anything too short.
    fn offends(&self, int: &str) -> bool {
        if int.starts_with('0') {
            return false;
        }
        if self.allowed_numbers.iter().any(|n| n == int) {
            return false;
        }
        if self.allowed_patterns.iter().any(|re| re.is_match(int)) {
            return false;
        }
        if int.len() < self.min_digits {
            return false;
        }

        let pure_digits = !int.is_empty() && int.bytes().all(|b| b.is_ascii_digit());
        pure_digits || has_four_consecutive_digits(int) || has_short_group(int, self.strict)
    }
}

/// RuboCop's `/\d{4}/` check: four consecutive ASCII digits anywhere.
fn has_four_consecutive_digits(s: &str) -> bool {
    let mut run = 0;
    for b in s.bytes() {
        if b.is_ascii_digit() {
            run += 1;
            if run >= 4 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// RuboCop's `short_group_regex`: an underscore followed by one or two
/// digits, then either another underscore (both styles) or the end of the
/// string (`Strict` only).
fn has_short_group(s: &str, strict: bool) -> bool {
    let bytes = s.as_bytes();
    let n = bytes.len();
    for i in 0..n {
        if bytes[i] != b'_' {
            continue;
        }
        let mut j = i + 1;
        let mut digits = 0;
        while j < n && digits < 2 && bytes[j].is_ascii_digit() {
            digits += 1;
            j += 1;
        }
        if digits == 0 {
            continue;
        }
        let followed_by_underscore = j < n && bytes[j] == b'_';
        let at_end = j == n;
        if followed_by_underscore || (strict && at_end) {
            return true;
        }
    }
    false
}

/// The byte offset of the first `e`, `E`, or `.` in `s` (RuboCop's
/// `DELIMITER_REGEXP`), if any.
fn find_delimiter(s: &str) -> Option<usize> {
    s.find(['e', 'E', '.'])
}

/// RuboCop's `IntegerNode#integer_part`: `s` with a single leading sign
/// stripped and everything from the first delimiter onward cut off.
fn integer_part(s: &str) -> &str {
    let unsigned = s.strip_prefix(['+', '-']).unwrap_or(s);
    match find_delimiter(unsigned) {
        Some(idx) => &unsigned[..idx],
        None => unsigned,
    }
}

/// RuboCop's `format_number`: strips all whitespace from `source` (collapsing
/// the widened whitespace-separated sign case back onto one line), then
/// re-delimits the integer part every three digits, leaving any
/// fractional/exponent part untouched.
fn format_number(source: &str) -> String {
    let stripped: String = source.chars().filter(|c| !c.is_whitespace()).collect();
    match find_delimiter(&stripped) {
        Some(idx) => {
            let int_part = format_int_part(&stripped[..idx]);
            let delimiter = &stripped[idx..=idx];
            let rest = &stripped[idx + 1..];
            format!("{int_part}{delimiter}{rest}")
        }
        None => format_int_part(&stripped),
    }
}

/// RuboCop's `format_int_part`: parses `int_part` as an (underscore-tolerant)
/// integer and re-renders it with an underscore every three digits from the
/// right, keeping a leading `-` (but never a leading `+`, matching
/// `Integer#to_s`) for negative values.
fn format_int_part(int_part: &str) -> String {
    let negative = int_part.starts_with('-');
    let digits: String = int_part.chars().filter(char::is_ascii_digit).collect();
    let grouped = group_thousands(&digits);
    if negative {
        format!("-{grouped}")
    } else {
        grouped
    }
}

/// Inserts `_` every three digits from the right of `digits`.
fn group_thousands(digits: &str) -> String {
    let bytes = digits.as_bytes();
    let n = bytes.len();
    let mut out = String::with_capacity(n + n / 3);
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (n - i) % 3 == 0 {
            out.push('_');
        }
        out.push(char::from(b));
    }
    out
}

/// Widens `node_span` to cover a bare unary `-`/`+` call's own span when
/// `node_span` is that call's sole receiver with nothing but the sign and
/// whitespace in between (see the module docs).
fn effective_span(node_span: Span, ctx: &Context<'_>) -> Span {
    let Some(NodeInfo { kind: NodeKind::CallNode, span: parent_span }) = ctx.parent() else {
        return node_span;
    };
    if parent_span.end != node_span.end || parent_span.start >= node_span.start {
        return node_span;
    }
    let gap = ctx.text(Span::new(parent_span.start, node_span.start));
    let Ok(gap) = std::str::from_utf8(gap) else { return node_span };
    let mut chars = gap.chars();
    match chars.next() {
        Some('+' | '-') => {}
        _ => return node_span,
    }
    if chars.all(|c| c.is_ascii_whitespace()) {
        parent_span
    } else {
        node_span
    }
}

/// Converts an `AllowedNumbers` entry (a YAML integer or string) to its
/// string form, matching RuboCop's `cop_config.fetch('AllowedNumbers',
/// []).map(&:to_s)`.
fn number_to_string(value: &OptionValue) -> Option<String> {
    match value {
        OptionValue::Str(s) => Some(s.clone()),
        OptionValue::Int(i) => Some(i.to_string()),
        _ => None,
    }
}

impl Rule for NumericLiterals {
    const META: RuleMeta = RuleMeta {
        name: "Style/NumericLiterals",
        department: Department::Style,
        summary: "Checks for big numeric literals without `_` between groups of digits in them.",
        explanation: "\
Additional allowed patterns can be added by adding regexps to the
`AllowedPatterns` configuration. All regexps are treated as anchored even if
the patterns do not contain anchors (so `\\d{4}_\\d{4}` will allow
`1234_5678` but not `1234_5678_9012`).

NOTE: Even if `AllowedPatterns` are given, autocorrection will only correct
to the standard pattern of an `_` every 3 digits.

```ruby
# bad
1000000
1_00_000
1_0000

# good
1_000_000
1000
```

```ruby
# Strict: false (default)

# good
10_000_00 # typical representation of $10,000 in cents
```

```ruby
# Strict: true

# bad
10_000_00 # typical representation of $10,000 in cents
```

```ruby
# AllowedNumbers: [3000]

# good
3000 # You can specify allowed numbers. (e.g. port number)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IntegerNode, NodeKind::FloatNode],
        config: &[
            ConfigOption {
                name: "MinDigits",
                default: ConfigDefault::Int(5),
                allowed: &[],
                doc: "The minimum number of (undelimited) digits that triggers an offense.",
            },
            ConfigOption {
                name: "Strict",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Also flag a trailing 1-2 digit group at the end of an otherwise \
delimited literal (e.g. `10_000_00`).",
            },
            ConfigOption {
                name: "AllowedNumbers",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Specific numbers (compared as written, digits only) exempted from \
this cop.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Regular expressions (anchored to the whole digit run) exempted from \
this cop.",
            },
        ],
        blind_spots: "\
Non-decimal literals (`0b`/`0o`/`0`/`0x` prefixes) and any float whose integer
part is a bare `0` are skipped by matching RuboCop's own `int.start_with?('0')`
guard verbatim, rather than checking the literal's actual base; this exactly
mirrors upstream's documented limitation (see the cop's `on_int` TODO) so it
is not a gap relative to RuboCop, but it does mean a huge `0.123456789` never
offends, same as real RuboCop. RuboCop's `case/when` distinguishes an
undelimited run from a misdelimited one only to steer its `--auto-gen-config`
suggestion (`MinDigits`/disabling the cop); since both arms always report the
same offense and fix, this port collapses them into one `offends` check and
never emits `--auto-gen-config` metadata (out of scope for a linter/fixer).
A literal that is the sole receiver of a bare unary `-`/`+` call (matching
RuboCop's real-parser sign fusion across whitespace/newlines) is widened to
the call's own span; a receiver reached through anything else (parentheses,
a further method chain, an explicit `.-@` call) is treated as an ordinary
literal instead of being folded in, which can only under- rather than
over-report relative to RuboCop's parser-level fusion.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let min_digits = usize::try_from(options.int("MinDigits")).unwrap_or(0);
        let strict = options.bool("Strict");

        let allowed_numbers = match options.get("AllowedNumbers") {
            Some(value) => match value.as_list() {
                Some(items) => items.iter().filter_map(number_to_string).collect(),
                None => number_to_string(value).into_iter().collect(),
            },
            None => Vec::new(),
        };

        let allowed_patterns = options
            .str_list("AllowedPatterns")
            .into_iter()
            .filter_map(|pattern| Regex::new(&format!("\\A(?:{pattern})\\z")).ok())
            .collect();

        Ok(Self { min_digits, allowed_numbers, allowed_patterns, strict })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let span = effective_span(node.span(), ctx);
        let Ok(source) = std::str::from_utf8(ctx.text(span)) else { return };

        let int = integer_part(source);
        if !self.offends(int) {
            return;
        }

        let fixed = format_number(source);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, fixed.into_bytes())],
            },
        );
    }
}
