//! `Style/OptionalBooleanParameter`, ported from RuboCop's
//! `lib/rubocop/cop/style/optional_boolean_parameter.rb` plus the
//! `AllowedMethods` mixin it includes.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// Checks for places where keyword arguments can be used instead of boolean
/// arguments when defining methods. `respond_to_missing?` is allowed by
/// default; customizable with `AllowedMethods`.
#[derive(Debug, Clone)]
pub struct OptionalBooleanParameter {
    /// RuboCop's `AllowedMethods` (`AllowedMethods` mixin).
    allowed_methods: Vec<String>,
}

impl Rule for OptionalBooleanParameter {
    const META: RuleMeta = RuleMeta {
        name: "Style/OptionalBooleanParameter",
        department: Department::Style,
        summary: "Checks for places where keyword arguments can be used instead of boolean \
                  arguments when defining methods.",
        explanation: "\
`respond_to_missing?` method is allowed by default. These are customizable
with `AllowedMethods` option.

```ruby
# bad
def some_method(bar = false)
  puts bar
end

# bad - common hack before keyword args were introduced
def some_method(options = {})
  bar = options.fetch(:bar, false)
  puts bar
end

# good
def some_method(bar: false)
  puts bar
end
```

With `AllowedMethods: ['some_method']`:

```ruby
# good
def some_method(bar = false)
  puts bar
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[linter::ConfigOption {
            name: "AllowedMethods",
            default: linter::ConfigDefault::StrList(&["respond_to_missing?"]),
            allowed: &[],
            doc: "Method names always allowed to take an optional boolean parameter.",
        }],
        blind_spots: "\
This cop is unsafe: changing a method signature from a positional boolean
default to a keyword argument implicitly changes call-site behaviour for
every existing positional caller. RuboCop reports it regardless (autocorrect
is simply never offered); this port matches that -- it never suppresses the
offense on safety grounds -- but, like RuboCop, does not attempt to fix it.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allowed_methods: options.str_list("AllowedMethods") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let def = node.as_def_node().expect("kind matched");
        let name = def.name();
        if self.allowed_methods.iter().any(|m| m.as_bytes() == name.as_slice()) {
            return;
        }
        let Some(params) = def.parameters() else { return };
        for arg in &params.optionals() {
            let Some(optarg) = arg.as_optional_parameter_node() else { continue };
            if !matches!(optarg.value(), Node::TrueNode { .. } | Node::FalseNode { .. }) {
                continue;
            }
            let original = String::from_utf8_lossy(ctx.text(optarg.location().span()));
            let default_source =
                String::from_utf8_lossy(ctx.text(optarg.value().location().span()));
            let replacement =
                format!("{}: {default_source}", String::from_utf8_lossy(optarg.name().as_slice()));
            let message = format!(
                "Prefer keyword arguments for arguments with a boolean default value; use \
                 `{replacement}` instead of `{original}`."
            );
            ctx.report(&Self::META, optarg.location().span(), message);
        }
    }
}
