//! `Rails/PluralizationGrammar`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/pluralization_grammar.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// `SINGULAR_METHODS` (singular, plural).
const METHODS: &[(&str, &str)] = &[
    ("second", "seconds"),
    ("minute", "minutes"),
    ("hour", "hours"),
    ("day", "days"),
    ("week", "weeks"),
    ("fortnight", "fortnights"),
    ("month", "months"),
    ("year", "years"),
    ("byte", "bytes"),
    ("kilobyte", "kilobytes"),
    ("megabyte", "megabytes"),
    ("gigabyte", "gigabytes"),
    ("terabyte", "terabytes"),
    ("petabyte", "petabytes"),
    ("exabyte", "exabytes"),
    ("zettabyte", "zettabytes"),
];

/// Checks for correct grammar when using `ActiveSupport`'s core extensions to
/// the numeric classes.
#[derive(Debug, Clone)]
pub struct PluralizationGrammar;

impl Rule for PluralizationGrammar {
    const META: RuleMeta = RuleMeta {
        name: "Rails/PluralizationGrammar",
        department: Department::Rails,
        summary: "Checks for incorrect grammar when using methods like `3.day.ago`.",
        explanation: "Checks for correct grammar when using `ActiveSupport`'s core extensions to \
                      the numeric classes.\n\n```ruby\n# bad\n3.day.ago\n1.months.ago\n\
                      5.megabyte\n1.gigabytes\n\n# good\n3.days.ago\n1.month.ago\n\
                      5.megabytes\n1.gigabyte\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return;
        }
        let name = call.name();
        let Ok(method) = std::str::from_utf8(name.as_slice()) else { return };
        let Some(&(singular, plural)) = METHODS.iter().find(|(s, p)| *s == method || *p == method)
        else {
            return;
        };
        let Some(receiver) = call.receiver() else { return };
        let Some(number) = Number::of(&receiver, ctx) else { return };

        let plural_method = method == plural;
        let singular_receiver = number.abs_is_one;
        if singular_receiver != plural_method {
            return;
        }
        let correct = if plural_method { singular } else { plural };
        let Some(selector) = call.message_loc() else { return };
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            format!("Prefer `{}.{correct}`.", number.text),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(selector.span(), correct.as_bytes().to_vec())],
            },
        );
    }
}

/// A literal `int` or `float` receiver.
struct Number {
    /// The number as Ruby's `to_s` renders it.
    text: String,
    abs_is_one: bool,
}

impl Number {
    fn of(node: &Node<'_>, ctx: &Context<'_>) -> Option<Self> {
        if let Some(float) = node.as_float_node() {
            let value = float.value();
            return Some(Self {
                text: ruby_float_to_s(value),
                abs_is_one: (value.abs() - 1.0).abs() < f64::EPSILON,
            });
        }
        node.as_integer_node()?;
        let source = String::from_utf8_lossy(ctx.text(node.span())).replace('_', "");
        let (negative, digits) = match source.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, source.strip_prefix('+').unwrap_or(&source)),
        };
        let lower = digits.to_ascii_lowercase();
        let (radix, body) = if let Some(rest) = lower.strip_prefix("0x") {
            (16, rest)
        } else if let Some(rest) = lower.strip_prefix("0b") {
            (2, rest)
        } else if let Some(rest) = lower.strip_prefix("0o").or_else(|| lower.strip_prefix("0d")) {
            (if lower.starts_with("0o") { 8 } else { 10 }, rest)
        } else if lower.len() > 1 && lower.starts_with('0') {
            (8, &lower[1..])
        } else {
            (10, lower.as_str())
        };
        let magnitude = u128::from_str_radix(body, radix).ok();
        let text = match magnitude {
            Some(value) => format!("{}{value}", if negative { "-" } else { "" }),
            None => source.clone(),
        };
        Some(Self { text, abs_is_one: magnitude == Some(1) })
    }
}

/// `Float#to_s`.
fn ruby_float_to_s(value: f64) -> String {
    let text = format!("{value:?}");
    let Some((mantissa, exponent)) = text.split_once('e') else { return text };
    let mantissa =
        if mantissa.contains('.') { mantissa.to_owned() } else { format!("{mantissa}.0") };
    let (sign, digits) = match exponent.strip_prefix('-') {
        Some(digits) => ('-', digits),
        None => ('+', exponent),
    };
    format!("{mantissa}e{sign}{digits:0>2}")
}
