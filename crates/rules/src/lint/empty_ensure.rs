//! `Lint/EmptyEnsure`, ported from RuboCop's
//! `lib/rubocop/cop/lint/empty_ensure.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Empty `ensure` block detected.";

/// Checks for empty `ensure` block.
#[derive(Debug, Clone)]
pub struct EmptyEnsure;

impl Rule for EmptyEnsure {
    const META: RuleMeta = RuleMeta {
        name: "Lint/EmptyEnsure",
        department: Department::Lint,
        summary: "Checks for empty ensure block.",
        explanation: "\
Checks for empty `ensure` blocks.

```ruby
# bad
def some_method
  do_something
ensure
end

# bad
begin
  do_something
ensure
end

# good
def some_method
  do_something
ensure
  do_something_else
end

# good
begin
  do_something
ensure
  do_something_else
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::EnsureNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(ensure_node) = node.as_ensure_node() else { return };
        if ensure_node.statements().is_some() {
            return;
        }

        let span = ensure_node.ensure_keyword_loc().span();
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] },
        );
    }
}
