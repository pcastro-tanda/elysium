//! `Style/RedundantPercentQ`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_percent_q.rb`.
//!
//! Upstream's `on_dstr`/`on_str` pair, restricted by `string_literal?`
//! (`node.loc?(:begin) && node.loc?(:end)`), maps directly onto Prism's
//! `InterpolatedStringNode`/`StringNode` with both `opening_loc` and
//! `closing_loc` present: a plain `%q(...)`/`%Q(...)` literal is a bare
//! `StringNode`; a `%Q(...)` with real interpolation is an
//! `InterpolatedStringNode` whose non-interpolated child `StringNode` parts
//! have no `opening_loc`/`closing_loc` of their own (so `string_literal?`
//! filters them out, matching upstream's comment about `on_dstr` firing for
//! the whole string and `on_str` firing separately only for genuinely
//! implicit, unquoted portions). Adjacent string-literal concatenation
//! (`%q(foo) \` newline `'bar'`) parses as an `InterpolatedStringNode` with
//! *no* opening/closing locs of its own, so only its individual quoted
//! `StringNode` parts (each with their own opening/closing locs) are
//! checked -- exactly matching upstream's per-literal offense there.

use std::sync::LazyLock;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::bytes::Regex;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG_STATIC: &str =
    "Use `%q` only for strings that contain both single quotes and double quotes.";
const MSG_DYNAMIC_SUFFIX: &str = ", or for dynamic strings that contain double quotes";

/// Checks for usage of the %q/%Q syntax when '' or "" would do.
#[derive(Debug, Clone)]
pub struct RedundantPercentQ;

impl Rule for RedundantPercentQ {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantPercentQ",
        department: Department::Style,
        summary: "Checks for usage of the %q/%Q syntax when '' or \"\" would do.",
        explanation: "Checks for usage of the %q/%Q syntax when '' or \"\" would do.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StringNode, NodeKind::InterpolatedStringNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (begin_span, end_span) = match node.kind() {
            NodeKind::StringNode => {
                let string = node.as_string_node().expect("kind matched");
                let Some(opening) = string.opening_loc() else { return };
                let Some(closing) = string.closing_loc() else { return };
                (opening.span(), closing.span())
            }
            NodeKind::InterpolatedStringNode => {
                let dstr = node.as_interpolated_string_node().expect("kind matched");
                let Some(opening) = dstr.opening_loc() else { return };
                let Some(closing) = dstr.closing_loc() else { return };
                (opening.span(), closing.span())
            }
            _ => return,
        };

        let span = node.span();
        let src = ctx.text(span);

        let is_q = src.starts_with(b"%q");
        let is_capital_q = src.starts_with(b"%Q");
        if !is_q && !is_capital_q {
            return;
        }

        if src.contains(&b'\'') && src.contains(&b'"') {
            return;
        }

        let is_str_type = node.kind() == NodeKind::StringNode;
        let allowed = if is_q { acceptable_q(src) } else { acceptable_capital_q(src, is_str_type) };
        if allowed {
            return;
        }

        let q_type = if is_capital_q { "%Q" } else { "%q" };
        let message = if is_capital_q {
            format!(
                "Use `{q_type}` only for strings that contain both single quotes and double \
                 quotes{MSG_DYNAMIC_SUFFIX}."
            )
        } else {
            MSG_STATIC.to_string()
        };

        // Upstream: `/\A%Q[^"]+\z|'/.match?(node.source) ? QUOTE : SINGLE_QUOTE`.
        let delimiter: &[u8] =
            if starts_with_capital_q_no_quotes(src) || src.contains(&b'\'') { b"\"" } else { b"'" };

        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![
                Edit::replace(begin_span, delimiter.to_vec()),
                Edit::replace(end_span, delimiter.to_vec()),
            ],
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }
}

/// `\A%Q[^"]+\z`: the whole source starts with `%Q` and contains no `"`
/// anywhere through to the end.
fn starts_with_capital_q_no_quotes(src: &[u8]) -> bool {
    src.starts_with(b"%Q") && src.len() > 2 && !src[2..].contains(&b'"')
}

/// RuboCop's `STRING_INTERPOLATION_REGEXP = /#\{.+\}/`.
fn string_interpolation_regexp() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\{.+\}").expect("valid regex"));
    &RE
}

/// RuboCop's `acceptable_q?`.
fn acceptable_q(src: &[u8]) -> bool {
    if string_interpolation_regexp().is_match(src) && src.contains(&b'\'') {
        return true;
    }
    has_escaped_non_backslash(src)
}

/// RuboCop's `acceptable_capital_q?`.
fn acceptable_capital_q(src: &[u8], is_str_type: bool) -> bool {
    src.contains(&b'"')
        && (string_interpolation_regexp().is_match(src)
            || (is_str_type && double_quotes_required(src)))
}

/// RuboCop's `src.scan(/\\./).any? { ESCAPED_NON_BACKSLASH === it }`, i.e.
/// "does the source contain a backslash-escape whose escaped character is
/// not itself a backslash", scanning left to right non-overlapping the same
/// way `String#scan` would.
fn has_escaped_non_backslash(src: &[u8]) -> bool {
    let mut i = 0;
    while i < src.len() {
        if src[i] == b'\\' && i + 1 < src.len() && src[i + 1] != b'\n' {
            if src[i + 1] != b'\\' {
                return true;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    false
}

/// RuboCop's `Util#double_quotes_required?`:
/// `/'|(?<! \\) \\{2}* \\ (?![\\"])/x`.
///
/// The `regex` crate has no lookaround, and a naive `(?:\\\\)*\\[^\\"]`
/// without the leading `(?<!\\)` would also wrongly match starting mid-run
/// (e.g. the second `\` of an escaped-backslash pair `\\` followed by a
/// plain letter). Backslash runs in a string are maximal and
/// non-overlapping, so scanning them directly is exactly equivalent to the
/// lookaround-anchored regex: an odd-length run has one dangling `\` that
/// escapes whatever follows it (or nothing, at end of string); that is a
/// "required" escape unless it is escaping a `"`.
fn double_quotes_required(src: &[u8]) -> bool {
    if src.contains(&b'\'') {
        return true;
    }
    let mut i = 0;
    while i < src.len() {
        if src[i] == b'\\' {
            let run_start = i;
            while i < src.len() && src[i] == b'\\' {
                i += 1;
            }
            if (i - run_start) % 2 == 1 && src.get(i) != Some(&b'"') {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}
