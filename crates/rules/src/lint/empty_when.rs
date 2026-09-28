//! `Lint/EmptyWhen`, ported from RuboCop's
//! `lib/rubocop/cop/lint/empty_when.rb`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Avoid `when` branches without a body.";

/// Checks for the presence of `when` branches without a body.
#[derive(Debug, Clone)]
pub struct EmptyWhen {
    allow_comments: bool,
}

impl Rule for EmptyWhen {
    const META: RuleMeta = RuleMeta {
        name: "Lint/EmptyWhen",
        department: Department::Lint,
        summary: "Checks for `when` branches with empty bodies.",
        explanation: "\
Checks for the presence of `when` branches without a body.

```ruby
# bad
case foo
when bar
  do_something
when baz
end

# good
case condition
when foo
  do_something
when bar
  nil
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CaseNode],
        config: &[ConfigOption {
            name: "AllowComments",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Allow a `when` branch to contain only comments.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_comments: options.bool("AllowComments") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(case_node) = node.as_case_node() else { return };
        let conditions: Vec<Node<'_>> = case_node.conditions().iter().collect();

        // RuboCop's `CommentsHelp#find_end_line`: the exclusive end line of a
        // `when` branch's own source range is the line of the next sibling
        // (the following `when`, or the `else` clause), or else the parent
        // `case`'s `end` line.
        let end_line = ctx.line_col(case_node.end_keyword_loc().span().start).line;

        for (i, branch) in conditions.iter().enumerate() {
            let Some(when_node) = branch.as_when_node() else { continue };
            if when_node.statements().is_some() {
                continue;
            }

            let start_line = ctx.line_col(when_node.keyword_loc().span().start).line;
            let next_line = if let Some(next) = conditions.get(i + 1) {
                ctx.line_col(next.span().start).line
            } else if let Some(else_node) = case_node.else_clause() {
                ctx.line_col(else_node.else_keyword_loc().span().start).line
            } else {
                end_line
            };

            if self.allow_comments
                && ctx
                    .comments()
                    .iter()
                    .any(|comment| comment.line >= start_line && comment.line < next_line)
            {
                continue;
            }

            // RuboCop's `add_offense(when_node)` highlights the `when`
            // keyword through the last condition, excluding any `then`
            // keyword, body, or trailing comment.
            let Some(last_condition) = when_node.conditions().last() else { continue };
            let span = Span::new(when_node.keyword_loc().span().start, last_condition.span().end);
            ctx.report(&Self::META, span, MSG);
        }
    }
}
