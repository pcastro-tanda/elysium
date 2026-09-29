//! `Style/PreferredHashMethods`, ported from RuboCop's
//! `lib/rubocop/cop/style/preferred_hash_methods.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Short,
    Verbose,
}

const MSG: &str = "Use `Hash#{prefer}` instead of `Hash#{current}`.";

/// Checks use of `has_key?` and `has_value?` Hash methods.
#[derive(Debug, Clone)]
pub struct PreferredHashMethods {
    style: Style,
}

impl Rule for PreferredHashMethods {
    const META: RuleMeta = RuleMeta {
        name: "Style/PreferredHashMethods",
        department: Department::Style,
        summary: "Checks use of `has_key?` and `has_value?` Hash methods.",
        explanation: "\
Checks for uses of methods `Hash#has_key?` and `Hash#has_value?`, and \
suggests using `Hash#key?` and `Hash#value?` instead.

It is configurable to enforce the verbose method names, by using the \
`EnforcedStyle: verbose` configuration.

@safety
  This cop is unsafe because it cannot be guaranteed that the receiver \
  is a `Hash` or responds to the replacement methods.

```ruby
# EnforcedStyle: short (default)
# bad
Hash#has_key?
Hash#has_value?

# good
Hash#key?
Hash#value?
```

```ruby
# EnforcedStyle: verbose
# bad
Hash#key?
Hash#value?

# good
Hash#has_key?
Hash#has_value?
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("short"),
            allowed: &["short", "verbose"],
            doc: "Whether to prefer the short (`key?`/`value?`) or verbose \
                  (`has_key?`/`has_value?`) `Hash` predicate method names.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "verbose" => Style::Verbose,
            _ => Style::Short,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let Some(args) = call.arguments() else { return };
        if args.arguments().len() != 1 {
            return;
        }
        let name = call.name();
        let name = name.as_slice();
        let offending = match self.style {
            Style::Short => matches!(name, b"has_key?" | b"has_value?"),
            Style::Verbose => matches!(name, b"key?" | b"value?"),
        };
        if !offending {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };
        let span = message_loc.span();

        let proper_name: &[u8] = match self.style {
            Style::Verbose => match name {
                b"key?" => b"has_key?",
                _ => b"has_value?",
            },
            Style::Short => match name {
                b"has_key?" => b"key?",
                _ => b"value?",
            },
        };
        let prefer = String::from_utf8_lossy(proper_name);
        let current = String::from_utf8_lossy(name);
        let message = MSG.replace("{prefer}", &prefer).replace("{current}", &current);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, proper_name.to_vec())],
            },
        );
    }
}
