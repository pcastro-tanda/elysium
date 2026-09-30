//! `Layout/SpaceInsideRangeLiteral`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_inside_range_literal.rb`.
//!
//! RuboCop's `on_irange`/`on_erange` (`..`/`...` are distinct whitequark node
//! types) collapse to Prism's single `RangeNode` (the two spellings differ
//! only by the `EXCLUDE_END` flag, irrelevant here since the check reads the
//! operator's own source text).
//!
//! Upstream builds a throwaway copy of the node's source, first collapsing
//! `<op>\n\s*` (the legitimate "trailing newline continues the range on the
//! next line" shape) to bare `<op>`, then testing whether a lone whitespace
//! character sits immediately before or after the operator in what remains.
//! The two are equivalent to inspecting the actual bytes touching the
//! operator directly: a byte immediately preceding it is only ever "the"
//! whitespace question (a single check suffices, matching the single-char
//! `\s` alternative), while a byte immediately following it is exempt only
//! when it is a literal `\n` (RuboCop's collapse is gated on that exact
//! byte, not on any leading whitespace character generally -- a `\r` alone,
//! e.g. bare CR line endings, still counts as an offending trailing space).
//! The fix mirrors this: whichever contiguous run of ASCII whitespace sits
//! on each side of the operator is deleted outright (Ruby's second `\s+`
//! substitution is a strict superset of the first `\n\s*` one, so a single
//! "delete the whole run" edit on each side reproduces both upstream `sub`s
//! byte-for-byte without ever reconstructing a whole-node replacement
//! string).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Space inside range literal.";

/// Ruby's `\s` character class: space, tab, newline, CR, form feed, vertical
/// tab.
fn is_ruby_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0C | 0x0B)
}

/// No spaces inside range literals.
#[derive(Debug, Clone)]
pub struct SpaceInsideRangeLiteral;

impl SpaceInsideRangeLiteral {
    fn check(ctx: &mut Context<'_>, node: &Node<'_>, range: &ruby_ast::node::RangeNode<'_>) {
        let node_span = node.span();
        let op_span = range.operator_loc().span();
        let bytes = ctx.source().bytes();

        let leading_offense = op_span.start > node_span.start
            && is_ruby_whitespace(bytes[(op_span.start - 1) as usize]);
        let trailing_offense = op_span.end < node_span.end
            && bytes[op_span.end as usize] != b'\n'
            && is_ruby_whitespace(bytes[op_span.end as usize]);

        if !leading_offense && !trailing_offense {
            return;
        }

        let mut lead_start = op_span.start;
        while lead_start > node_span.start && is_ruby_whitespace(bytes[(lead_start - 1) as usize]) {
            lead_start -= 1;
        }
        let mut trail_end = op_span.end;
        while trail_end < node_span.end && is_ruby_whitespace(bytes[trail_end as usize]) {
            trail_end += 1;
        }

        let mut edits = Vec::with_capacity(2);
        if lead_start < op_span.start {
            edits.push(Edit::delete(Span::new(lead_start, op_span.start)));
        }
        if trail_end > op_span.end {
            edits.push(Edit::delete(Span::new(op_span.end, trail_end)));
        }
        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, node_span, MSG, fix);
    }
}

impl Rule for SpaceInsideRangeLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceInsideRangeLiteral",
        department: Department::Layout,
        summary: "Checks for spaces inside range literals.",
        explanation: "\
```ruby
# bad
1 .. 3

# good
1..3

# bad
'a' .. 'z'

# good
'a'..'z'
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::RangeNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let range = node.as_range_node().expect("RangeNode kind");
        Self::check(ctx, node, &range);
    }
}
