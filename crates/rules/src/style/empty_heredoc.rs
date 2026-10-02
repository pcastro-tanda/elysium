//! `Style/EmptyHeredoc`, ported from RuboCop's
//! `lib/rubocop/cop/style/empty_heredoc.rb`.
//!
//! Prism models a heredoc string's whole-token span (`node.span()`) as
//! covering only the opening delimiter (e.g. `<<~EOS`), with the body in
//! `content_loc` and the closing delimiter line in `closing_loc` -- this
//! lines up exactly with whitequark's separate `heredoc_body`/`heredoc_end`
//! locations alongside `node.loc.expression` covering the opening token, so
//! the offense range and replace/remove edits below map one for one onto
//! upstream's.
//!
//! A backtick heredoc (`<<~` followed by a backtick-delimited tag) parses
//! as an `XStringNode`/
//! `InterpolatedXStringNode`, which this rule's `kinds` list excludes,
//! matching upstream's `return if node.xstr_type?`. An interpolated,
//! non-empty heredoc parses as `InterpolatedStringNode`, also excluded,
//! since it can never have an empty body.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "Use an empty string literal instead of heredoc.";

/// Checks for using empty heredoc to reduce redundancy.
#[derive(Debug, Clone)]
pub struct EmptyHeredoc {
    /// `StringLiteralsHelp#preferred_string_literal`: `Style/StringLiterals`'s
    /// peered `EnforcedStyle`, `''` unless it is `double_quotes`.
    preferred_string_literal: &'static [u8],
}

impl Rule for EmptyHeredoc {
    const META: RuleMeta = RuleMeta {
        name: "Style/EmptyHeredoc",
        department: Department::Style,
        summary: "Checks for using empty heredoc to reduce redundancy.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StringNode],
        config: &[],
        blind_spots: "Reads `Style/StringLiterals`'s `EnforcedStyle` as a peer option to pick the \
                      replacement literal's quote style.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let double_quotes = options
            .peer("Style/StringLiterals", "EnforcedStyle")
            .and_then(linter::OptionValue::as_str)
            .is_some_and(|style| style == "double_quotes");
        Ok(Self { preferred_string_literal: if double_quotes { b"\"\"" } else { b"''" } })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !ext::is_heredoc(node) {
            return;
        }
        let string = node.as_string_node().expect("kind matched");
        let body_span = string.content_loc().span();
        if !body_span.is_empty() {
            return;
        }
        let Some(closing) = string.closing_loc() else { return };
        let closing_span = closing.span();

        let body_lines = ctx.whole_lines(body_span);
        let closing_lines = ctx.whole_lines(closing_span);

        let mut edits = vec![Edit::replace(node.span(), self.preferred_string_literal.to_vec())];
        edits.push(Edit::delete(closing_lines));
        if body_lines != closing_lines {
            edits.push(Edit::delete(body_lines));
        }

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            MSG,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}
