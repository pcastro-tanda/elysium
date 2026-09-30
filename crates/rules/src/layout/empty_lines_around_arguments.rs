//! `Layout/EmptyLinesAroundArguments`, ported from RuboCop's
//! `lib/rubocop/cop/layout/empty_lines_around_arguments.rb`.
//!
//! Upstream's `on_send` is aliased to `on_csend`, so this port subscribes
//! to `CallNode` regardless of [`CallNode::is_safe_navigation`] the same
//! way.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// RuboCop's `MSG`.
const MSG: &str = "Empty line detected around arguments.";

/// Keeps track of empty lines around method arguments.
#[derive(Debug, Clone)]
pub struct EmptyLinesAroundArguments;

impl Rule for EmptyLinesAroundArguments {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLinesAroundArguments",
        department: Department::Layout,
        summary: "Keeps track of empty lines around method arguments.",
        explanation: "\
A blank line right after a call's opening parenthesis, between two
arguments, or right before the closing parenthesis is noise:

```ruby
# bad
do_something(
  foo

)

# good
do_something(
  foo
)
```

The fix removes each such blank line.",
        enabled_by_default: true,
        severity: Severity::Convention,
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
        let call = node.as_call_node().expect("kind matched");
        if ctx.is_single_line(node.span()) {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let arguments = args.arguments();
        if arguments.is_empty() {
            return;
        }
        if let Some(receiver) = call.receiver() {
            let receiver_last_line = ctx.last_line(receiver.span());
            let same_line = call
                .message_loc()
                .is_some_and(|loc| ctx.line_col(loc.span().start).line == receiver_last_line);
            if !same_line {
                return;
            }
        }

        for arg in &arguments {
            check_starting_point(ctx, Span::empty(arg.span().start));
        }
        if let Some(closing) = call.closing_loc() {
            check_starting_point(ctx, Span::empty(closing.span().start));
        }
    }
}

/// RuboCop's `empty_range_for_starting_point`: when the whitespace
/// immediately preceding `start` spans more than one line, the blank line
/// right above `start`'s own line (newline included) is the offense.
fn check_starting_point(ctx: &mut Context<'_>, start: Span) {
    let range = ctx.with_surrounding_space(start, Side::Left, false, true);
    let first_line = ctx.line_col(range.start).line;
    // RuboCop's `Range#last_line` is `source_buffer.line_for_position(end_pos)`:
    // the line the *position* `end_pos` sits at, not the line of the byte just
    // before it. That matters here specifically because `range.end` is
    // unshifted (only the left side is expanded), so when the following
    // token (a closing paren, most commonly) begins its own line at column 0,
    // `range.end` lands exactly on that line's start -- `Context::last_line`'s
    // `end - 1` convention would (wrongly, for this one call site) still
    // count that as the *previous*, blank line.
    let last_line = ctx.line_col(range.end).line;
    if last_line <= first_line + 1 {
        return;
    }
    let blank_line = last_line - 1;
    let removal = Span::new(ctx.line_span(blank_line).start, ctx.line_span(blank_line + 1).start);
    ctx.report_with_fix(
        &EmptyLinesAroundArguments::META,
        removal,
        MSG,
        Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(removal)] },
    );
}
