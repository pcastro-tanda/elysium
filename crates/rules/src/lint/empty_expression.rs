//! `Lint/EmptyExpression`, ported from RuboCop's
//! `lib/rubocop/cop/lint/empty_expression.rb`.
//!
//! Whitequark's parser wraps *any* parenthesized expression -- including an
//! empty `()` -- in a `begin` node, so upstream's `on_begin` simply checks
//! `begin_node.children.empty?`. Prism instead gives an empty `()` a
//! `ParenthesesNode` whose `body` is absent (`None`) and a non-empty one a
//! `ParenthesesNode` whose `body` is the single contained statement (or a
//! `StatementsNode` wrapping several, for `(a; b)`) directly, with no
//! intermediate empty-children case to check: subscribing to
//! `ParenthesesNode` and testing `body().is_none()` reproduces
//! `empty_expression?` exactly, including the nested case (`(())`, whose
//! *inner* `ParenthesesNode` is the one with no body and is the one
//! reported, matching upstream) and every conditional/return/assignment
//! position from the spec, since Prism represents `()` identically in all
//! of them.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Avoid empty expressions.";

/// Checks for the presence of empty expressions.
#[derive(Debug, Clone, Default)]
pub struct EmptyExpression;

impl Rule for EmptyExpression {
    const META: RuleMeta = RuleMeta {
        name: "Lint/EmptyExpression",
        department: Department::Lint,
        summary: "Checks for the presence of empty expressions.",
        explanation: "\
Checks for the presence of empty expressions.

```ruby
# bad

foo = ()
if ()
  bar
end

# good

foo = (some_expression)
if (some_expression)
  bar
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ParenthesesNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(parens) = node.as_parentheses_node() else { return };
        if parens.body().is_none() {
            ctx.report(&Self::META, node.span(), MSG);
        }
    }
}
