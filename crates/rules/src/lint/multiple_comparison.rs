//! `Lint/MultipleComparison`, ported from RuboCop's
//! `lib/rubocop/cop/lint/multiple_comparison.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Use the `&&` operator to compare multiple values.";

/// RuboCop's `COMPARISON_METHODS`.
const COMPARISON_METHODS: &[&[u8]] = &[b"<", b">", b"<=", b">="];

/// RuboCop's `SET_OPERATION_OPERATORS`.
const SET_OPERATION_OPERATORS: &[&[u8]] = &[b"&", b"|", b"^"];

/// Use `&&` operator to compare multiple values.
#[derive(Debug, Clone)]
pub struct MultipleComparison;

impl Rule for MultipleComparison {
    const META: RuleMeta = RuleMeta {
        name: "Lint/MultipleComparison",
        department: Department::Lint,
        summary: "Use `&&` operator to compare multiple values.",
        explanation: "\
In math and Python, we can use `x < y < z` style comparison to compare
multiple value. However, we can't use the comparison in Ruby. However,
the comparison is not syntax error. This cop checks the bad usage of
comparison operators.

```ruby
# bad
x < y < z
10 <= x <= 20

# good
x < y && y < z
10 <= x && x <= 20
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
        let Some(outer) = node.as_call_node() else { return };
        if !is_comparison(&outer) {
            return;
        }
        let Some(receiver) = outer.receiver() else { return };
        let Some(inner) = receiver.as_call_node() else { return };
        if !is_comparison(&inner) {
            return;
        }
        let Some(center) = single_argument(&inner) else { return };
        // It allows multiple comparison using `&`, `|`, and `^` set
        // operation operators, e.g. `x >= y & y < z`.
        if let Some(center_call) = center.as_call_node() {
            if SET_OPERATION_OPERATORS.contains(&center_call.name().as_slice()) {
                return;
            }
        }

        let span = node.span();
        let center_span = center.span();
        let center_source = ctx.text(center_span).to_vec();
        let mut replacement = center_source.clone();
        replacement.extend_from_slice(b" && ");
        replacement.extend_from_slice(&center_source);

        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(center_span, replacement)],
            },
        );
    }
}

/// Whether `call` is a plain (non-safe-navigation) comparison send.
fn is_comparison(call: &CallNode<'_>) -> bool {
    !call.is_safe_navigation() && COMPARISON_METHODS.contains(&call.name().as_slice())
}

/// The call's single positional argument, if it has exactly one.
fn single_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let arguments = call.arguments()?.arguments();
    if arguments.len() == 1 {
        arguments.iter().next()
    } else {
        None
    }
}
