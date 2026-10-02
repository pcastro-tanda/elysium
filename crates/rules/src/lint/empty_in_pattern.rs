//! `Lint/EmptyInPattern`, ported from RuboCop's
//! `lib/rubocop/cop/lint/empty_in_pattern.rb`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Avoid `in` branches without a body.";

/// Checks for the presence of `in` pattern branches without a body.
#[derive(Debug, Clone)]
pub struct EmptyInPattern {
    allow_comments: bool,
}

impl Rule for EmptyInPattern {
    const META: RuleMeta = RuleMeta {
        name: "Lint/EmptyInPattern",
        department: Department::Lint,
        summary: "Checks for the presence of `in` pattern branches without a body.",
        explanation: "\
Checks for the presence of `in` pattern branches without a body.

```ruby
# bad
case condition
in [a]
  do_something
in [a, b]
end

# good
case condition
in [a]
  do_something
in [a, b]
  nil
end
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CaseMatchNode],
        config: &[ConfigOption {
            name: "AllowComments",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Allow an `in` branch to contain only comments.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_comments: options.bool("AllowComments") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(case_match) = node.as_case_match_node() else { return };
        let conditions: Vec<Node<'_>> = case_match.conditions().iter().collect();

        // RuboCop's `CommentsHelp#find_end_line`: the exclusive end line of
        // an `in` branch's own source range is the line of the next sibling
        // (the following `in`, or the `else` clause), or else the parent
        // `case`'s `end` line.
        let end_line = ctx.line_col(case_match.end_keyword_loc().span().start).line;

        for (i, branch) in conditions.iter().enumerate() {
            let Some(in_node) = branch.as_in_node() else { continue };
            if in_node.statements().is_some() {
                continue;
            }

            let start_line = ctx.line_col(in_node.in_loc().span().start).line;
            let next_line = if let Some(next) = conditions.get(i + 1) {
                ctx.line_col(next.span().start).line
            } else if let Some(else_node) = case_match.else_clause() {
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

            // RuboCop's `add_offense(branch)` highlights the `in` keyword
            // through the pattern, excluding any `then` keyword, body, or
            // trailing comment.
            let span = Span::new(in_node.in_loc().span().start, in_node.pattern().span().end);
            ctx.report(&Self::META, span, MSG);
        }
    }
}
