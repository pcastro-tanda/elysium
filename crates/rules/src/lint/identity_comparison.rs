//! `Lint/IdentityComparison`, ported from RuboCop's
//! `lib/rubocop/cop/lint/identity_comparison.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{node::CallNode, Node, NodeExt as _, NodeKind};

/// Prefer `equal?` over `==` when comparing `object_id`.
#[derive(Debug, Clone)]
pub struct IdentityComparison;

impl Rule for IdentityComparison {
    const META: RuleMeta = RuleMeta {
        name: "Lint/IdentityComparison",
        department: Department::Lint,
        summary: "Prefer `equal?` over `==` when comparing `object_id`.",
        explanation: "\
`Object#equal?` is provided to compare objects for identity, and in contrast
`Object#==` is provided for the purpose of doing value comparison.

```ruby
# bad
foo.object_id == bar.object_id
foo.object_id != baz.object_id

# good
foo.equal?(bar)
!foo.equal?(baz)
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name().as_slice();
        let is_eq = name == b"==";
        if !is_eq && name != b"!=" {
            return;
        }

        let Some(receiver) = call.receiver() else { return };
        let Some(receiver_call) = receiver.as_call_node() else { return };
        if !is_object_id_call(&receiver_call) {
            return;
        }

        let Some(arguments) = call.arguments() else { return };
        let args = arguments.arguments();
        if args.len() != 1 {
            return;
        }
        let Some(single_arg) = args.iter().next() else { return };
        let Some(argument_call) = single_arg.as_call_node() else { return };
        if !is_object_id_call(&argument_call) {
            return;
        }

        let bang = if is_eq { "" } else { "!" };
        let comparison_method = if is_eq { "==" } else { "!=" };
        let message = format!(
            "Use `{bang}equal?` instead of `{comparison_method}` when comparing `object_id`."
        );

        let span = node.span();
        let fix = match (receiver_call.receiver(), argument_call.receiver()) {
            (Some(lhs), Some(rhs)) => {
                let lhs_text = String::from_utf8_lossy(ctx.text(lhs.span())).into_owned();
                let rhs_text = String::from_utf8_lossy(ctx.text(rhs.span())).into_owned();
                let replacement = format!("{bang}{lhs_text}.equal?({rhs_text})");
                Some(Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(span, replacement.into_bytes())],
                })
            }
            _ => None,
        };

        match fix {
            Some(fix) => ctx.report_with_fix(&Self::META, span, message, fix),
            None => ctx.report(&Self::META, span, message),
        }
    }
}

/// A receiver-having call to `object_id` with no arguments.
fn is_object_id_call(call: &CallNode<'_>) -> bool {
    call.name().as_slice() == b"object_id"
        && call.receiver().is_some()
        && call.arguments().is_none_or(|a| a.arguments().is_empty())
}
