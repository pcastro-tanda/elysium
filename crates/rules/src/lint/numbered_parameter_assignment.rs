//! `Lint/NumberedParameterAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/lint/numbered_parameter_assignment.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// Upstream's `NUM_PARAM_MSG`.
const NUM_PARAM_MSG: &str =
    "`_{number}` is reserved for numbered parameter; consider another name.";
/// Upstream's `LVAR_MSG`.
const LVAR_MSG: &str = "`_{number}` is similar to numbered parameter; consider another name.";

/// Checks for uses of numbered parameter assignment.
#[derive(Debug, Clone)]
pub struct NumberedParameterAssignment;

impl Rule for NumberedParameterAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Lint/NumberedParameterAssignment",
        department: Department::Lint,
        summary: "Checks for uses of numbered parameter assignment.",
        explanation: "\
It emulates the following warning in Ruby 2.7:

    $ ruby -ve '_1 = :value'
    ruby 2.7.2p137 (2020-10-01 revision 5445e04352) [x86_64-darwin19]
    -e:1: warning: `_1' is reserved for numbered parameter; consider another name

Assigning to a numbered parameter (from `_1` to `_9`) causes an error in Ruby 3.0.

    $ ruby -ve '_1 = :value'
    ruby 3.0.0p0 (2020-12-25 revision 95aff21468) [x86_64-darwin19]
    -e:1: _1 is reserved for numbered parameter

NOTE: The numbered parameters are from `_1` to `_9`. This cop checks `_0`, and over `_10`
as well to prevent confusion.

```ruby
# bad
_1 = :value

# good
non_numbered_parameter_name = :value
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::LocalVariableWriteNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(n) = node.as_local_variable_write_node() else { return };
        let name = n.name();
        let name = name.as_slice();
        let Some(digits) = name.strip_prefix(b"_") else { return };
        if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
            return;
        }
        let Ok(digits_str) = std::str::from_utf8(digits) else { return };
        let Ok(number) = digits_str.parse::<u32>() else { return };
        let template = if (1..=9).contains(&number) { NUM_PARAM_MSG } else { LVAR_MSG };
        let message = template.replacen("{number}", &number.to_string(), 1);
        ctx.report(&Self::META, node.span(), message);
    }
}
