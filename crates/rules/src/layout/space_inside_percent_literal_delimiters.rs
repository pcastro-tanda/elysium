//! `Layout/SpaceInsidePercentLiteralDelimiters`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_inside_percent_literal_delimiters.rb`, plus
//! the `PercentLiteral` and `MatchRange` mixins it uses.
//!
//! Upstream only defines `on_array` (`%i`/`%I`/`%w`/`%W`, filtered by
//! `PercentLiteral#process` exactly like
//! `Layout/SpaceInsideArrayPercentLiteral`; [`array_percent_type`] is
//! shared with that cop) and `on_xstr` (`%x` command literals only --
//! whitequark's single `:xstr` node type covers both Prism's
//! [`NodeKind::XStringNode`] and [`NodeKind::InterpolatedXStringNode`], the
//! same split `Style/CommandLiteral` handles, but `PercentLiteral#process`'s
//! `percent_literal?` filter drops a plain backtick literal -- its
//! `begin_source` is `` "`" ``, which doesn't start with `%` -- so this cop
//! never touches backtick literals at all (see
//! `accepts_execute_string_literals`). `%q`/`%Q`/`%r`/`%s` are never visited
//! either (see `accepts_other_percent_literals`).
//!
//! `on_percent_literal` runs two independent checks against `body_range`
//! (the raw source strictly between the opening and closing delimiter):
//!
//! - `add_offenses_for_blank_spaces`: when the entire body is non-empty and
//!   consists only of whitespace (`source.strip.empty?`), one offense
//!   covers the whole body, removed entirely -- this fires even across a
//!   newline (`%w(\n)`).
//! - `add_offenses_for_unnecessary_spaces`: only when the node is
//!   single-line, `BEGIN_REGEX = /\A( +)/` and
//!   `END_REGEX = /(?<!\\)( +)\z/` each contribute their own offense (a
//!   leading space run has no escape exception; a trailing run's negative
//!   lookbehind means one space directly preceded by a backslash --
//!   `\ `'s escaped space -- is left alone, only the *further* excess
//!   trailing spaces beyond it are flagged).
//!
//! Both checks can produce the exact same byte range (e.g. a single-space
//! blank body matches the blank check *and* both `BEGIN_REGEX`/`END_REGEX`);
//! upstream's `Base#add_offense` silently drops an offense whose range a
//! prior one already claimed (`current_offense_locations.add?(range)`), so
//! [`dedup_push`] reproduces that de-duplication by hand.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{is_ruby_whitespace, Span};
use std::ops::Range;

use super::space_inside_array_percent_literal::array_percent_type;

/// RuboCop's `MSG`.
const MSG: &str = "Do not use spaces inside percent literal delimiters.";

/// Narrows a byte offset back to `u32`, saturating instead of panicking;
/// files large enough to overflow `u32` are not a concern here.
fn u32_of(x: usize) -> u32 {
    u32::try_from(x).unwrap_or(u32::MAX)
}

/// No unnecessary spaces inside delimiters of %i/%w/%x literals.
#[derive(Debug, Clone)]
pub struct SpaceInsidePercentLiteralDelimiters;

impl Rule for SpaceInsidePercentLiteralDelimiters {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceInsidePercentLiteralDelimiters",
        department: Department::Layout,
        summary: "No unnecessary spaces inside delimiters of %i/%w/%x literals.",
        explanation: "Checks for unnecessary additional spaces inside the delimiters of \
                      %i/%w/%x literals.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ArrayNode, NodeKind::XStringNode, NodeKind::InterpolatedXStringNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (opening_span, closing_span) = match node {
            Node::ArrayNode { .. } => {
                let array = node.as_array_node().expect("kind matched");
                let (Some(opening), Some(closing)) = (array.opening_loc(), array.closing_loc())
                else {
                    return;
                };
                let opening_span = opening.span();
                if array_percent_type(ctx.text(opening_span)).is_none() {
                    return;
                }
                (opening_span, closing.span())
            }
            Node::XStringNode { .. } => {
                let n = node.as_x_string_node().expect("kind matched");
                let opening_span = n.opening_loc().span();
                if !ctx.text(opening_span).starts_with(b"%x") {
                    return;
                }
                (opening_span, n.closing_loc().span())
            }
            Node::InterpolatedXStringNode { .. } => {
                let n = node.as_interpolated_x_string_node().expect("kind matched");
                let opening_span = n.opening_loc().span();
                if !ctx.text(opening_span).starts_with(b"%x") {
                    return;
                }
                (opening_span, n.closing_loc().span())
            }
            _ => return,
        };

        let content_span = Span::new(opening_span.end, closing_span.start);
        let body = ctx.text(content_span);

        let mut ranges: Vec<Range<usize>> = Vec::new();

        if !body.is_empty() && body.iter().all(|&b| is_ruby_whitespace(b)) {
            dedup_push(&mut ranges, 0..body.len());
        }

        if ctx.is_single_line(node.span()) {
            if let Some(r) = leading_space_run(body) {
                dedup_push(&mut ranges, r);
            }
            if let Some(r) = trailing_space_run(body) {
                dedup_push(&mut ranges, r);
            }
        }

        for r in ranges {
            let span =
                Span::new(content_span.start + u32_of(r.start), content_span.start + u32_of(r.end));
            let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] };
            ctx.report_with_fix(&Self::META, span, MSG, fix);
        }
    }
}

/// `BEGIN_REGEX = /\A( +)/`: the run of one or more spaces starting at
/// byte 0, if any.
fn leading_space_run(body: &[u8]) -> Option<Range<usize>> {
    if body.first() != Some(&b' ') {
        return None;
    }
    let mut n = 0;
    while n < body.len() && body[n] == b' ' {
        n += 1;
    }
    Some(0..n)
}

/// `END_REGEX = /(?<!\\)( +)\z/`: the maximal run of spaces ending the
/// body, minus one space if that run is itself immediately preceded by a
/// backslash (an escaped space, `\ `, which the negative lookbehind
/// protects) -- leaving `None` when the whole run was that single escaped
/// space.
fn trailing_space_run(body: &[u8]) -> Option<Range<usize>> {
    if body.last() != Some(&b' ') {
        return None;
    }
    let mut start = body.len();
    while start > 0 && body[start - 1] == b' ' {
        start -= 1;
    }
    if start > 0 && body[start - 1] == b'\\' {
        start += 1;
    }
    (start < body.len()).then_some(start..body.len())
}

/// Reproduces upstream's `Base#add_offense`
/// (`current_offense_locations.add?(range)`): only the first offense
/// registered at a given byte range survives.
fn dedup_push(ranges: &mut Vec<Range<usize>>, r: Range<usize>) {
    if !ranges.contains(&r) {
        ranges.push(r);
    }
}
