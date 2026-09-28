//! `Lint/FloatOutOfRange`, ported from RuboCop's
//! `lib/rubocop/cop/lint/float_out_of_range.rb`.
//!
//! Upstream reads `node.value`, the whitequark parser's own pre-parsed
//! `Float` for the literal (computed once, at parse time, via Ruby's own
//! `Float()` conversion). This port instead re-parses the literal's own
//! source text (with `_` digit separators stripped, matching how Ruby's
//! lexer treats them as invisible) through Rust's `f64::from_str`, which
//! implements the same round-to-nearest `strtod`-style algorithm and so
//! overflows to infinity / underflows to zero on exactly the same inputs;
//! no discrepancy is expected (or reproducible) between the two for any
//! literal Ruby's own lexer accepts.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Float out of range.";

/// Checks for `Float` literals too large or small for Ruby to represent.
#[derive(Debug, Clone)]
pub struct FloatOutOfRange;

impl Rule for FloatOutOfRange {
    const META: RuleMeta = RuleMeta {
        name: "Lint/FloatOutOfRange",
        department: Department::Lint,
        summary: "Catches floating-point literals too large or small for Ruby to represent.",
        explanation: "\
Identifies `Float` literals which are, like, really really really
really really really really big. Too big. No-one needs Floats
that big. If you need a float that big, something is wrong with you.

```ruby
# bad
float = 3.0e400

# good
float = 42.9
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::FloatNode],
        config: &[],
        blind_spots: "\
The literal's source text is re-parsed with Rust's `f64::from_str` after
stripping `_` digit separators, rather than reusing a pre-parsed value from
the parser; this matches Ruby's own `Float()` semantics (round-to-nearest,
overflow to infinity, underflow to zero) for every literal Ruby's lexer
accepts, so no discrepancy is expected.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let span = node.span();
        let Ok(source) = std::str::from_utf8(ctx.text(span)) else { return };
        let cleaned = source.replace('_', "");
        let Ok(value) = cleaned.parse::<f64>() else { return };

        let offends = value.is_infinite()
            || (value == 0.0 && source.bytes().any(|b| b.is_ascii_digit() && b != b'0'));
        if !offends {
            return;
        }

        ctx.report(&Self::META, span, MSG);
    }
}
