//! `Layout/SpaceInsideArrayPercentLiteral`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_inside_array_percent_literal.rb`, plus the
//! `PercentLiteral` and `MatchRange` mixins it uses.
//!
//! Upstream drives this off
//! `MULTIPLE_SPACES_BETWEEN_ITEMS_REGEX = /(?:[\S&&[^\\]](?:\\ )*)( {2,})(?=\S)/`
//! scanned over the literal's contents (the raw source between its opening
//! and closing delimiter, from `PercentLiteral#process`/`on_percent_literal`).
//! Rust's `regex` crate has no lookaround, so [`multiple_space_runs`]
//! reimplements the same greedy left-to-right match by hand: a start byte
//! that is neither whitespace nor a backslash, then zero or more `\ `
//! (escaped-space, used inside `%w`/`%W` items to embed a literal space)
//! pairs, then a run of two or more literal spaces immediately followed by
//! a non-whitespace byte. Each such run becomes its own offense, replaced
//! with a single space, matching `each_unnecessary_space_match`/
//! `corrector.replace(range, ' ')`.
//!
//! `percent_literal?`/`type(node)` (`node.loc.begin.source[0..-2]`) narrow
//! the array node down to the four kinds this cop cares about
//! (`%i`/`%I`/`%w`/`%W`); [`array_percent_type`] is reused by
//! `Layout/SpaceInsidePercentLiteralDelimiters`'s `on_array` for the exact
//! same filter.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::{is_ruby_whitespace, Span};

/// RuboCop's `MSG`.
const MSG: &str = "Use only a single space inside array percent literal.";

/// `PercentLiteral#type`/`percent_literal?` narrowed to array literals:
/// the opening delimiter's source with its trailing bracket character
/// stripped, when that leaves one of the four array percent-literal kinds
/// this cop (and `SpaceInsidePercentLiteralDelimiters`'s `on_array`) cares
/// about. Returns `None` for a bracketed `[...]` array, or any other
/// percent-literal kind (`%q`/`%Q`/`%r`/`%s`/`%x`).
pub(crate) fn array_percent_type(opening_text: &[u8]) -> Option<&[u8]> {
    if opening_text.first() != Some(&b'%') || opening_text.len() < 2 {
        return None;
    }
    let ty = &opening_text[..opening_text.len() - 1];
    matches!(ty, b"%i" | b"%I" | b"%w" | b"%W").then_some(ty)
}

/// Narrows a byte offset back to `u32`, saturating instead of panicking;
/// files large enough to overflow `u32` are not a concern here.
fn u32_of(x: usize) -> u32 {
    u32::try_from(x).unwrap_or(u32::MAX)
}

/// No unnecessary additional spaces between elements in %i/%w literals.
#[derive(Debug, Clone)]
pub struct SpaceInsideArrayPercentLiteral;

impl Rule for SpaceInsideArrayPercentLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceInsideArrayPercentLiteral",
        department: Department::Layout,
        summary: "No unnecessary additional spaces between elements in %i/%w literals.",
        explanation: "Checks for unnecessary additional spaces inside array percent literals \
                      (i.e. %i/%w).\n\nNote that blank percent literals (e.g. `%i( )`) are \
                      checked by `Layout/SpaceInsidePercentLiteralDelimiters`.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ArrayNode],
        config: &[],
        blind_spots: "The escaped-space scan is a hand-rolled stand-in for a lookaround regex \
                      Rust's engine can't express; it always resolves each candidate start \
                      byte with the same one choice the real regex's backtracking converges \
                      on for ordinary `%w`/`%i` items (maximal `\\ ` consumption, then the \
                      longest immediately-following space run), rather than exploring every \
                      backtrack path, so a contrived item mixing runs of escaped and \
                      unescaped spaces in a way that requires giving back consumed `\\ \\ ` \
                      pairs to find a match could be scored differently.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Node::ArrayNode { .. } = node else { return };
        let array = node.as_array_node().expect("kind matched");
        let (Some(opening), Some(closing)) = (array.opening_loc(), array.closing_loc()) else {
            return;
        };
        let opening_span = opening.span();
        if array_percent_type(ctx.text(opening_span)).is_none() {
            return;
        }

        let content_span = Span::new(opening_span.end, closing.span().start);
        let body = ctx.text(content_span);
        for run in multiple_space_runs(body) {
            let span = Span::new(
                content_span.start + u32_of(run.start),
                content_span.start + u32_of(run.end),
            );
            let fix =
                Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(span, *b" ")] };
            ctx.report_with_fix(&Self::META, span, MSG, fix);
        }
    }
}

/// See the module doc: a manual reimplementation of
/// `MULTIPLE_SPACES_BETWEEN_ITEMS_REGEX`'s greedy left-to-right match,
/// since the `regex` crate has no lookaround support.
fn multiple_space_runs(body: &[u8]) -> Vec<std::ops::Range<usize>> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < body.len() {
        if is_ruby_whitespace(body[i]) || body[i] == b'\\' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j + 1 < body.len() && body[j] == b'\\' && body[j + 1] == b' ' {
            j += 2;
        }
        let space_start = j;
        while j < body.len() && body[j] == b' ' {
            j += 1;
        }
        if j - space_start >= 2 && j < body.len() && !is_ruby_whitespace(body[j]) {
            runs.push(space_start..j);
            i = j;
        } else {
            i += 1;
        }
    }
    runs
}
