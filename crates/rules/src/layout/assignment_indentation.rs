//! `Layout/AssignmentIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/assignment_indentation.rb` plus the
//! `CheckAssignment` and `Alignment` mixins it includes.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::end_keyword_alignment::{
    assignment_operator, extract_rhs, is_assignment_kind, is_setter_call, logical_parent,
    ASSIGNMENT_KINDS,
};

/// RuboCop's `MSG`.
const MSG: &str = "Indent the first line of the right-hand-side of a multi-line assignment.";

/// Checks the indentation of the first line of the right-hand-side of a multi-line assignment.
#[derive(Debug, Clone)]
pub struct AssignmentIndentation {
    /// `Alignment#configured_indentation_width`.
    width: i64,
    /// RuboCop's per-investigation `@current_offenses`: an offense whose
    /// range sits inside one already reported is emitted without a fix,
    /// since two rewrites of the same region in one pass cannot be handled.
    reported: Vec<Span>,
    /// Whether each open `CallNode` ancestor is a setter call, which
    /// `SendNode#assignment?` is an alias for. Outermost first.
    setter_calls: Vec<bool>,
}

impl Rule for AssignmentIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/AssignmentIndentation",
        department: Department::Layout,
        summary: "Checks the indentation of the first line of the right-hand-side of a multi-line assignment.",
        explanation: "\
The first line of a right-hand side that starts on its own line is indented
one `IndentationWidth` past the start of the assignment.

```ruby
# bad
value =
if foo
  'bar'
end

# good
value =
  if foo
    'bar'
  end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: ASSIGNMENT_KINDS,
        config: &[ConfigOption {
            name: "IndentationWidth",
            default: ConfigDefault::Nil,
            allowed: &[],
            doc: "Number of spaces the right-hand side is indented by, overriding \
                  `Layout/IndentationWidth`'s `Width` (which itself defaults to 2).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let width = options
            .get("IndentationWidth")
            .and_then(OptionValue::as_int)
            .or_else(|| {
                options.peer("Layout/IndentationWidth", "Width").and_then(OptionValue::as_int)
            })
            .unwrap_or(2);
        Ok(Self { width, reported: Vec::new(), setter_calls: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Node::CallNode { .. } = node {
            let call = node.as_call_node().expect("kind matched");
            let setter = is_setter_call(&call);
            let safe_navigation = call.is_safe_navigation();
            self.check(node, ctx, safe_navigation);
            self.setter_calls.push(setter);
            return;
        }
        self.check(node, ctx, false);
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if let Node::CallNode { .. } = node {
            self.setter_calls.pop();
        }
    }
}

impl AssignmentIndentation {
    /// RuboCop's `check_assignment`.
    fn check(&mut self, node: &Node<'_>, ctx: &mut Context<'_>, safe_navigation: bool) {
        // `CheckAssignment#on_send` is not aliased to `on_csend`.
        if safe_navigation {
            return;
        }
        let Some(rhs) = extract_rhs(node) else { return };
        let Some(operator) = assignment_operator(node) else { return };
        let rhs_span = rhs.span();
        if ctx.same_line(operator, rhs_span) {
            return;
        }
        // `Alignment#each_bad_alignment`, with a single-element `items`.
        if !ctx.begins_its_line(rhs_span) {
            return;
        }
        let node_span = node.span();
        let base = i64::from(ctx.display_column(self.leftmost_assignment(ctx, node_span).start));
        let delta = base + self.width - i64::from(ctx.display_column(rhs_span.start));
        if delta == 0 {
            return;
        }
        let within_reported =
            self.reported.iter().any(|o| rhs_span.start >= o.start && rhs_span.end <= o.end);
        self.reported.push(rhs_span);
        if within_reported {
            ctx.report(&Self::META, rhs_span, MSG);
            return;
        }
        let taboo = linter::heredoc_bodies(ctx, &rhs);
        let edits = linter::shift_lines(ctx, rhs_span, i32::try_from(delta).unwrap_or(0), &taboo);
        if edits.is_empty() {
            ctx.report(&Self::META, rhs_span, MSG);
        } else {
            let fix = Fix { applicability: Applicability::Safe, edits };
            ctx.report_with_fix(&Self::META, rhs_span, MSG, fix);
        }
    }

    /// RuboCop's `leftmost_multiple_assignment`: one level up while the
    /// assignment is nested directly in another one that starts on the same
    /// line (the upstream recursion discards its own result and returns
    /// `node.parent`, so it never climbs further).
    fn leftmost_assignment(&self, ctx: &Context<'_>, node_span: Span) -> Span {
        let Some(parent) = logical_parent(ctx) else { return node_span };
        let parent_is_assignment = if parent.kind == NodeKind::CallNode {
            self.setter_calls.last().copied().unwrap_or(false)
        } else {
            is_assignment_kind(parent.kind)
        };
        if ctx.same_line(node_span, parent.span) && parent_is_assignment {
            parent.span
        } else {
            node_span
        }
    }
}
