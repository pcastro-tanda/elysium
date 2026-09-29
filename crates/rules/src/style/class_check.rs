//! `Style/ClassCheck`, ported from RuboCop's
//! `lib/rubocop/cop/style/class_check.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    IsA,
    KindOf,
}

/// Enforces consistent use of `Object#is_a?` or `Object#kind_of?`.
#[derive(Debug, Clone)]
pub struct ClassCheck {
    style: Style,
}

impl Rule for ClassCheck {
    const META: RuleMeta = RuleMeta {
        name: "Style/ClassCheck",
        department: Department::Style,
        summary: "Enforces consistent use of `Object#is_a?` or `Object#kind_of?`.",
        explanation: "\
```ruby
# EnforcedStyle: is_a? (default)
# bad
var.kind_of?(Date)
var.kind_of?(Integer)

# good
var.is_a?(Date)
var.is_a?(Integer)
```

```ruby
# EnforcedStyle: kind_of?
# bad
var.is_a?(Time)
var.is_a?(String)

# good
var.kind_of?(Time)
var.kind_of?(String)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("is_a?"),
            allowed: &["is_a?", "kind_of?"],
            doc: "Whether to prefer `Object#is_a?` or `Object#kind_of?`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "kind_of?" => Style::KindOf,
            _ => Style::IsA,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let name = name.as_slice();
        let is_a = name == b"is_a?";
        let kind_of = name == b"kind_of?";
        if !is_a && !kind_of {
            return;
        }
        let current_matches_style =
            (is_a && self.style == Style::IsA) || (kind_of && self.style == Style::KindOf);
        if current_matches_style {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };
        let span = message_loc.span();

        let (prefer, current, replacement): (&str, &str, &[u8]) =
            if is_a { ("kind_of?", "is_a?", b"kind_of?") } else { ("is_a?", "kind_of?", b"is_a?") };
        let message = format!("Prefer `Object#{prefer}` over `Object#{current}`.");
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement.to_vec())],
            },
        );
    }
}
