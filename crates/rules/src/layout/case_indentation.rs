//! `Layout/CaseIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/case_indentation.rb` plus the `Alignment` and
//! `ConfigurableEnforcedStyle` mixins it includes (the latter's
//! `correct_style_detected`/`opposite_style_detected`/
//! `unrecognized_style_detected` only drive `--auto-gen-config` bookkeeping,
//! never this file's own offense/fix decisions, so they are not ported).
//!
//! Prism gives `case`/`case_match` an `else_clause` child instead of
//! whitequark's own `loc.else` field directly on the case node, and a
//! `when`/`in` branch's `loc.begin` (used as the "last conditional" marker
//! when there is no `else`) is its `then_keyword_loc`/`then_loc` -- present
//! only when that branch actually spells out `then`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `style_parameter_name` (`EnforcedStyle`): whether `when`/`in`
/// is measured against `case` or `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Case,
    End,
}

/// Checks how the `when` and `in` clauses of a `case` expression are
/// indented in relation to its `case` or `end` keyword.
#[derive(Debug, Clone)]
pub struct CaseIndentation {
    style: Style,
    /// `indentation_width`: `0` unless `IndentOneStep`, in which case this
    /// is `Alignment#configured_indentation_width`.
    indent_offset: u32,
}

impl CaseIndentation {
    /// RuboCop's `base_column`.
    fn base_column(&self, ctx: &Context<'_>, case_kw: Span, end_kw: Span) -> u32 {
        let start = match self.style {
            Style::Case => case_kw.start,
            Style::End => end_kw.start,
        };
        ctx.line_col(start).column
    }

    /// RuboCop's `end_and_last_conditional_same_line?`. `last_marker` is
    /// the `else` clause's own `else` keyword when there is one, else the
    /// last branch's `then`/`in ... then` keyword location (`nil` when
    /// that branch has no `then` at all, matching upstream's
    /// `child_nodes.last.loc.begin&.line`).
    fn end_and_last_same_line(ctx: &Context<'_>, end_kw: Span, last_marker: Option<Span>) -> bool {
        let Some(marker) = last_marker else { return false };
        ctx.line_col(end_kw.start).line == ctx.line_col(marker.start).line
    }

    /// RuboCop's `check_when` plus `incorrect_style`/`whitespace_range`/
    /// `replacement`: reports (and, where the branch keyword begins its
    /// own line, fixes) one `when`/`in` branch.
    fn check_when(
        &self,
        ctx: &mut Context<'_>,
        keyword_span: Span,
        case_kw: Span,
        end_kw: Span,
        branch_type: &str,
    ) {
        let when_column = ctx.line_col(keyword_span.start).column;
        let base = self.base_column(ctx, case_kw, end_kw);
        let desired = base + self.indent_offset;
        if when_column == desired {
            return;
        }

        let depth = if self.indent_offset > 0 { "one step more than" } else { "as deep as" };
        let base_name = match self.style {
            Style::Case => "case",
            Style::End => "end",
        };
        let message = format!("Indent `{branch_type}` {depth} `{base_name}`.");

        let line = ctx.line_col(keyword_span.start).line;
        let line_start = ctx.line_span(line).start;
        let whitespace = Span::new(line_start, keyword_span.start);

        if ctx.text(whitespace).iter().all(u8::is_ascii_whitespace) {
            let replacement = vec![b' '; usize::try_from(desired).unwrap_or(0)];
            ctx.report_with_fix(
                &<Self as Rule>::META,
                keyword_span,
                message,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(whitespace, replacement)],
                },
            );
        } else {
            ctx.report(&<Self as Rule>::META, keyword_span, message);
        }
    }
}

impl Rule for CaseIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/CaseIndentation",
        department: Department::Layout,
        summary: "Checks how the `when` and `in` clauses of a `case` expression are indented.",
        explanation: "\
Checks how the `when` and `in` clauses of a `case` expression are indented in
relation to its `case` or `end` keyword. It will register a separate offense
for each misaligned `when` and `in`.

If `Layout/EndAlignment` is set to keyword style (default), `case` and `end`
should always be aligned to the same depth, and therefore `when` should
always be aligned to both -- regardless of configuration.

With `EnforcedStyle: case` (the default), `when`/`in` is measured against the
`case` keyword's own column; with `EnforcedStyle: end`, against the `end`
keyword's column (only meaningful when `Layout/EndAlignment`'s
`EnforcedStyleAlignWith` is set to something other than `keyword`).
`IndentOneStep` shifts the expected column one further step (this cop's
`Layout/IndentationWidth`, or its own `IndentationWidth` override) past the
base.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CaseNode, NodeKind::CaseMatchNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("case"),
                allowed: &["case", "end"],
                doc: "Whether `when`/`in` is measured against `case` or `end`.",
            },
            ConfigOption {
                name: "IndentOneStep",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether `when`/`in` should be indented one step further than the base, rather than the same depth.",
            },
            ConfigOption {
                name: "IndentationWidth",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "Overrides `Layout/IndentationWidth`'s configured width for \
                      `IndentOneStep`'s extra step; falls back to it, else 2.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = if options.style("EnforcedStyle")? == "end" { Style::End } else { Style::Case };
        let indent_one_step = options.bool("IndentOneStep");
        let indent_offset = if indent_one_step {
            let width = options
                .get("IndentationWidth")
                .and_then(OptionValue::as_int)
                .or_else(|| {
                    options.peer("Layout/IndentationWidth", "Width").and_then(OptionValue::as_int)
                })
                .unwrap_or(2);
            u32::try_from(width).unwrap_or(2)
        } else {
            0
        };
        Ok(Self { style, indent_offset })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::CaseNode { .. } => {
                let case_node = node.as_case_node().expect("kind matched");
                if ctx.is_single_line(node.span()) {
                    return;
                }
                let case_kw = case_node.case_keyword_loc().span();
                let end_kw = case_node.end_keyword_loc().span();
                let conditions = case_node.conditions();
                let last_marker = match case_node.else_clause() {
                    Some(else_node) => Some(else_node.else_keyword_loc().span()),
                    None => conditions
                        .iter()
                        .last()
                        .and_then(|n| n.as_when_node())
                        .and_then(|w| w.then_keyword_loc())
                        .map(|loc| loc.span()),
                };
                if self.style == Style::End
                    && Self::end_and_last_same_line(ctx, end_kw, last_marker)
                {
                    return;
                }
                for cond in &conditions {
                    let Some(when_node) = cond.as_when_node() else { continue };
                    self.check_when(ctx, when_node.keyword_loc().span(), case_kw, end_kw, "when");
                }
            }
            Node::CaseMatchNode { .. } => {
                let case_node = node.as_case_match_node().expect("kind matched");
                if ctx.is_single_line(node.span()) {
                    return;
                }
                let case_kw = case_node.case_keyword_loc().span();
                let end_kw = case_node.end_keyword_loc().span();
                let conditions = case_node.conditions();
                let last_marker = match case_node.else_clause() {
                    Some(else_node) => Some(else_node.else_keyword_loc().span()),
                    None => conditions
                        .iter()
                        .last()
                        .and_then(|n| n.as_in_node())
                        .and_then(|w| w.then_loc())
                        .map(|loc| loc.span()),
                };
                if self.style == Style::End
                    && Self::end_and_last_same_line(ctx, end_kw, last_marker)
                {
                    return;
                }
                for cond in &conditions {
                    let Some(in_node) = cond.as_in_node() else { continue };
                    self.check_when(ctx, in_node.in_loc().span(), case_kw, end_kw, "in");
                }
            }
            _ => {}
        }
    }
}
