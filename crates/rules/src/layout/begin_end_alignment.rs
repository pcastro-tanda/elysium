//! `Layout/BeginEndAlignment`, ported from RuboCop's
//! `lib/rubocop/cop/layout/begin_end_alignment.rb` plus the
//! `EndKeywordAlignment` mixin it includes.
//!
//! Prism has no `kwbegin` node: an explicit `begin ... end` and the implicit
//! wrapper a `def`/`class`/block body grows when it carries a
//! `rescue`/`ensure` clause are both [`NodeKind::BeginNode`]. Only the
//! former has a `begin` keyword, so `begin_keyword_loc` is what stands in
//! for upstream's `on_kwbegin`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

use super::end_keyword_alignment::{
    align_end, end_is_aligned, misalignment_message, start_line_range, uses_tabs,
};

/// RuboCop's `EnforcedStyleAlignWith`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Align `end` with the first non-blank character of the `begin` line.
    StartOfLine,
    /// Align `end` with the `begin` keyword itself.
    Begin,
}

/// Align ends corresponding to begins correctly.
#[derive(Debug, Clone)]
pub struct BeginEndAlignment {
    style: Style,
    tabs: bool,
}

impl Rule for BeginEndAlignment {
    const META: RuleMeta = RuleMeta {
        name: "Layout/BeginEndAlignment",
        department: Department::Layout,
        summary: "Align ends corresponding to begins correctly.",
        explanation: "\
`Layout/EndAlignment` aligns with keywords by default; `||= begin` tends to
align with the start of its line instead, so this cop defaults to
`EnforcedStyleAlignWith: start_of_line`.

```ruby
# bad (start_of_line)
foo ||= begin
          do_something
        end

# good (start_of_line)
foo ||= begin
  do_something
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::BeginNode],
        config: &[ConfigOption {
            name: "EnforcedStyleAlignWith",
            default: ConfigDefault::Str("start_of_line"),
            allowed: &["start_of_line", "begin"],
            doc: "Whether `end` lines up with the start of the line the `begin` keyword is on \
                  (`start_of_line`) or with the `begin` keyword itself (`begin`).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyleAlignWith")? {
            "begin" => Style::Begin,
            _ => Style::StartOfLine,
        };
        Ok(Self { style, tabs: uses_tabs(options) })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let begin = node.as_begin_node().expect("kind matched");
        let Some(begin_kw) = begin.begin_keyword_loc() else { return };
        let Some(end_kw) = begin.end_keyword_loc() else { return };
        let end_loc = end_kw.span();
        let align_with = match self.style {
            Style::Begin => begin_kw.span(),
            Style::StartOfLine => start_line_range(ctx, node.span()),
        };
        if end_is_aligned(ctx, align_with, end_loc) {
            return;
        }
        let message = misalignment_message(ctx, end_loc, align_with);
        // `alignment_node` picks the `begin` node itself for the `begin`
        // style and the same `start_line_range` for `start_of_line`; both
        // start where `align_with` does.
        let column = ctx.line_col(align_with.start).column;
        match align_end(ctx, end_loc, column, self.tabs) {
            Some(edit) => ctx.report_with_fix(
                &Self::META,
                end_loc,
                message,
                Fix { applicability: Applicability::Safe, edits: vec![edit] },
            ),
            None => ctx.report(&Self::META, end_loc, message),
        }
    }
}
