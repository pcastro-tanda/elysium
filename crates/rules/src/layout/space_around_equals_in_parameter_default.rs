//! `Layout/SpaceAroundEqualsInParameterDefault`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_around_equals_in_parameter_default.rb`.
//!
//! Upstream's `on_optarg` grabs the node's first three lexer tokens (the
//! argument name, the `=`, and whatever token starts the default value
//! expression) and checks `space_after?` between name/`=` and between
//! `=`/value. Prism's [`ruby_ast::node::OptionalParameterNode`] exposes the
//! same three positions directly as locations -- `name_loc`, `operator_loc`,
//! and the default value node's own span (whose start coincides with that
//! third token: a unary-signed literal like `-1`/`+1` parses as a `CallNode`
//! whose span begins at the sign, exactly where upstream's third token
//! would) -- so no token stream is needed here, just three location
//! comparisons.
//!
//! Upstream's `autocorrect` additionally regex-matches `/=\s*(\S+)/` against
//! the offense range's source to preserve any non-whitespace text the range
//! might contain before replacing it. That range only ever spans from the
//! name's end to the value's start, i.e. the `=` plus incidental whitespace
//! -- it never reaches into the value token itself -- so the regex can never
//! actually capture anything in practice; this port replaces the range with
//! the plain `" = "`/`"="` replacement directly.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`, with `type: 'missing'`.
const MSG_MISSING: &str = "Surrounding space missing in default value assignment.";
/// RuboCop's `MSG`, with `type: 'detected'`.
const MSG_DETECTED: &str = "Surrounding space detected in default value assignment.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Space,
    NoSpace,
}

/// Checks that the equals signs in parameter default assignments have or
/// don't have surrounding space depending on configuration.
#[derive(Debug, Clone)]
pub struct SpaceAroundEqualsInParameterDefault {
    style: Style,
}

impl Rule for SpaceAroundEqualsInParameterDefault {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceAroundEqualsInParameterDefault",
        department: Department::Layout,
        summary: "Checks that the equals signs in parameter default assignments have or don't \
                  have surrounding space depending on configuration.",
        explanation: "\
```ruby
# EnforcedStyle: space (default)

# bad
def some_method(arg1=:default, arg2=nil, arg3=[])
  # do something...
end

# good
def some_method(arg1 = :default, arg2 = nil, arg3 = [])
  # do something...
end
```

```ruby
# EnforcedStyle: no_space

# bad
def some_method(arg1 = :default, arg2 = nil, arg3 = [])
  # do something...
end

# good
def some_method(arg1=:default, arg2=nil, arg3=[])
  # do something...
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::OptionalParameterNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("space"),
            allowed: &["space", "no_space"],
            doc: "Whether a parameter's default-value `=` requires or forbids surrounding space.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "no_space" => Style::NoSpace,
            _ => Style::Space,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let NodeKind::OptionalParameterNode = node.kind() else { return };
        let Some(param) = node.as_optional_parameter_node() else { return };

        let name_span = param.name_loc().span();
        let operator_span = param.operator_loc().span();
        let value_span = param.value().span();

        // RuboCop's `space_on_both_sides?`/`no_surrounding_space?`, each
        // half checked as "is there a gap between these two adjacent
        // positions" rather than scanning for whitespace bytes (nothing but
        // whitespace can occupy either gap).
        let space_before = operator_span.start > name_span.end;
        let space_after = value_span.start > operator_span.end;

        let correct = match self.style {
            Style::Space => space_before && space_after,
            Style::NoSpace => !space_before && !space_after,
        };
        if correct {
            return;
        }

        let range = Span::new(name_span.end, value_span.start);
        let (message, replacement): (&'static str, &[u8]) = match self.style {
            Style::Space => (MSG_MISSING, b" = "),
            Style::NoSpace => (MSG_DETECTED, b"="),
        };
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, replacement)],
            },
        );
    }
}
