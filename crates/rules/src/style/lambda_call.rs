//! `Style/LambdaCall`, ported from RuboCop's
//! `lib/rubocop/cop/style/lambda_call.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Call,
    Braces,
}

/// Use lambda.call(...) instead of lambda.(...).
#[derive(Debug, Clone)]
pub struct LambdaCall {
    style: Style,
    /// Spans of nodes whose fix has already been applied in this file, in
    /// traversal order. Mirrors RuboCop's `ignore_node`/`part_of_ignored_node?`:
    /// rewriting a call rebuilds its receiver as a literal string, so a nested
    /// offending call inside an already-fixed receiver must not also be
    /// autocorrected (it would double-edit the same bytes).
    claimed: Vec<Span>,
}

impl Rule for LambdaCall {
    const META: RuleMeta = RuleMeta {
        name: "Style/LambdaCall",
        department: Department::Style,
        summary: "Use lambda.call(...) instead of lambda.(...).",
        explanation: "\
```ruby
# EnforcedStyle: call (default)

# bad
lambda.(x, y)

# good
lambda.call(x, y)
```

```ruby
# EnforcedStyle: braces

# bad
lambda.call(x, y)

# good
lambda.(x, y)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("call"),
            allowed: &["call", "braces"],
            doc: "Whether to prefer `lambda.call(...)` or `lambda.(...)`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "braces" => Style::Braces,
            _ => Style::Call,
        };
        Ok(Self { style, claimed: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let Some(receiver) = call.receiver() else { return };
        let span = node.span();

        // Rewriting the call rebuilds it as a single expression, which would drop any
        // comments inside the argument list, so leave it to be fixed manually.
        if ctx.comments().iter().any(|c| span.start <= c.span.start && c.span.end <= span.end) {
            return;
        }

        // `implicit_call?`: the `lambda.(...)` syntax has no `call` selector text.
        let implicit_call = call.message_loc().is_none();
        let offense = match self.style {
            Style::Call => implicit_call,
            Style::Braces => !implicit_call,
        };
        if !offense {
            return;
        }

        let Some(dot_loc) = call.call_operator_loc() else { return };
        let receiver_src = String::from_utf8_lossy(ctx.text(receiver.span()));
        let dot = String::from_utf8_lossy(ctx.text(dot_loc.span()));
        let joined_args = call
            .arguments()
            .map(|args| {
                args.arguments()
                    .iter()
                    .map(|a| String::from_utf8_lossy(ctx.text(a.span())).into_owned())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        let method = match self.style {
            Style::Call => {
                if joined_args.is_empty() {
                    "call".to_string()
                } else {
                    format!("call({joined_args})")
                }
            }
            Style::Braces => format!("({joined_args})"),
        };
        let prefer = format!("{receiver_src}{dot}{method}");
        let current = String::from_utf8_lossy(ctx.text(span));
        let message = format!("Prefer the use of `{prefer}` over `{current}`.");

        let contained = self.claimed.iter().any(|c| c.start <= span.start && span.end <= c.end);
        if contained {
            ctx.report(&Self::META, span, message);
            return;
        }
        self.claimed.push(span);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, prefer.into_bytes())],
            },
        );
    }
}
