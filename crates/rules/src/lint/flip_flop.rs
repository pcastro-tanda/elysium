//! `Lint/FlipFlop`, ported from RuboCop's
//! `lib/rubocop/cop/lint/flip_flop.rb`.
//!
//! Whitequark's parser only produces its `iflipflop`/`eflipflop` node types
//! for a `..`/`...` range literal that the parser itself recognizes as being
//! in boolean context (an `if`/`unless`/`while`/ternary condition, etc.);
//! upstream's `on_iflipflop`/`on_eflipflop` simply flag every occurrence of
//! either. Prism performs the identical boolean-context detection at parse
//! time and, when it applies, emits a dedicated `FlipFlopNode` instead of the
//! ordinary `RangeNode` -- so subscribing to `FlipFlopNode` and reporting on
//! every occurrence reproduces upstream exactly, with no context-walking
//! needed on this port's side.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Avoid the use of flip-flop operators.";

/// Checks for flip-flop operators (`(a..b)`/`(a...b)` used in a boolean
/// context, e.g. as an `if` condition).
#[derive(Debug, Clone, Default)]
pub struct FlipFlop;

impl Rule for FlipFlop {
    const META: RuleMeta = RuleMeta {
        name: "Lint/FlipFlop",
        department: Department::Lint,
        summary: "Checks for flip-flops.",
        explanation: "\
Looks for uses of flip-flop operator based on the Ruby Style Guide.

Here is the history of flip-flops in Ruby. Flip-flop operator is deprecated
in Ruby 2.6.0 and the deprecation has been reverted by Ruby 2.7.0 and
backported to Ruby 2.6. See: https://bugs.ruby-lang.org/issues/5400

```ruby
# bad
(1..20).each do |x|
  puts x if (x == 5) .. (x == 10)
end

# good
(1..20).each do |x|
  puts x if (x >= 5) && (x <= 10)
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::FlipFlopNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        ctx.report(&Self::META, node.span(), MSG);
    }
}
