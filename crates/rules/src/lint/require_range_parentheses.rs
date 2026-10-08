//! `Lint/RequireRangeParentheses`, ported from RuboCop's
//! `lib/rubocop/cop/lint/require_range_parentheses.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// Upstream's `MSG`.
const MSG: &str =
    "Wrap the range literal `{range}` in parentheses to avoid confusion with an endless range.";

/// Checks that a range literal is enclosed in parentheses when the end of the range is at a line break.
#[derive(Debug, Clone)]
pub struct RequireRangeParentheses;

impl Rule for RequireRangeParentheses {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RequireRangeParentheses",
        department: Department::Lint,
        summary: "Checks that a range literal is enclosed in parentheses when the end of the \
                   range is at a line break.",
        explanation: "\
NOTE: The following is maybe intended for `(42..)`. But, compatible is `42..do_something`.
So, this cop does not provide autocorrection because it is left to user.

```ruby
case condition
when 42..
  do_something
end
```

```ruby
# bad - Represents `(1..42)`, not endless range.
1..
42

# good - It's incompatible, but your intentions when using endless range may be:
(1..)
42

# good
1..42

# good
(1..42)

# good
(1..
42)
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::RangeNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if in_parentheses(ctx) {
            return;
        }
        let Some(range) = node.as_range_node() else { return };
        let (Some(begin), Some(end)) = (range.left(), range.right()) else { return };
        let operator = range.operator_loc().span();
        if ctx.same_line(operator, end.span()) {
            return;
        }
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(ctx.text(begin.span())),
            String::from_utf8_lossy(ctx.text(operator))
        );
        let message = MSG.replacen("{range}", &text, 1);
        ctx.report(&Self::META, node.span(), message);
    }
}

/// Whether the current node's (possibly `StatementsNode`-wrapped) parent is
/// a `ParenthesesNode` -- Prism's equivalent of whitequark's `begin_type?`
/// grouping node, which always wraps even a single statement.
fn in_parentheses(ctx: &Context<'_>) -> bool {
    let ancestors = ctx.ancestors();
    match ancestors.last() {
        Some(last) if last.kind == NodeKind::ParenthesesNode => true,
        Some(last) if last.kind == NodeKind::StatementsNode => ancestors
            .get(ancestors.len().wrapping_sub(2))
            .is_some_and(|a| a.kind == NodeKind::ParenthesesNode),
        _ => false,
    }
}
