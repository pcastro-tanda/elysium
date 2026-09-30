//! `Layout/FirstParameterIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/first_parameter_indentation.rb` plus the
//! `Alignment`, `ConfigurableEnforcedStyle`, and `MultilineElementIndentation`
//! mixins it includes.
//!
//! Unlike `Layout/FirstArrayElementIndentation`/`Layout/FirstHashElementIndentation`,
//! this cop never runs `MultilineElementIndentation#indent_base`'s
//! `left_parenthesis`/`:special_inside_parentheses` branch (`check` always
//! passes `nil` for it) and never reaches its `:parent_hash_key` branch
//! either (a `def`'s own parameter list is never itself the value of a hash
//! pair), so `indent_base` collapses to exactly the two cases spelled out in
//! `EnforcedStyle`'s `SupportedStyles`: `align_parentheses` and `consistent`.
//! This port therefore inlines that reduced `indent_base` directly rather
//! than reusing `Layout/FirstHashElementIndentation`'s fuller machinery.
//!
//! A Prism `DefNode` covers both `def foo` and `def self.foo` (whitequark's
//! `def`/`defs`), so one `NodeKind::DefNode` arm replaces the upstream
//! `on_def`/`on_defs` alias pair.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{is_ruby_whitespace_char, Span};

use super::line_length::def_parameter_list;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Consistent,
    AlignParentheses,
}

/// RuboCop's `indent_base_type` return values, driving both the offense
/// message and `autocorrect_incompatible_with_other_cops?`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaseType {
    LeftParenthesis,
    StartOfLine,
}

impl BaseType {
    /// RuboCop's `base_description`.
    fn description(self) -> &'static str {
        match self {
            BaseType::LeftParenthesis => "the position of the opening parenthesis",
            BaseType::StartOfLine => "the start of the line where the left parenthesis is",
        }
    }
}

/// RuboCop's `MSG`.
fn message(width: i64, base_description: &str) -> String {
    format!("Use {width} spaces for indentation in method args, relative to {base_description}.")
}

/// Checks the indentation of the first parameter in a method definition,
/// ported from RuboCop's `FirstParameterIndentation` cop plus its
/// `Alignment` and `ConfigurableEnforcedStyle` mixins.
#[derive(Debug, Clone)]
pub struct FirstParameterIndentation {
    style: Style,
    indentation_width: i64,
    /// `Layout/ParameterAlignment`'s `EnforcedStyle == 'with_fixed_indentation'`.
    fixed_indentation_conflict: bool,
}

impl Rule for FirstParameterIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/FirstParameterIndentation",
        department: Department::Layout,
        summary: "Checks the indentation of the first parameter in a method definition.",
        explanation: "\
Checks the indentation of the first parameter in a method definition.
Parameters after the first one are checked by `Layout/ParameterAlignment`,
not by this cop.

```ruby
# bad
def some_method(
first_param,
second_param)
  123
end
```

```ruby
# EnforcedStyle: consistent (default)
# The first parameter should always be indented one step more than the
# preceding line.

# good
def some_method(
  first_param,
second_param)
  123
end
```

```ruby
# EnforcedStyle: align_parentheses
# The first parameter should always be indented one step more than the
# opening parenthesis.

# good
def some_method(
                 first_param,
second_param)
  123
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("consistent"),
                allowed: &["consistent", "align_parentheses"],
                doc: "Whether the first parameter's indentation is always relative to the \
                      start of the line where the left parenthesis is (`consistent`), or \
                      relative to the opening parenthesis's own column \
                      (`align_parentheses`).",
            },
            ConfigOption {
                name: "IndentationWidth",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "Number of spaces for the first parameter's indentation, overriding \
                      `Layout/IndentationWidth`'s `Width` (which itself defaults to 2).",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "align_parentheses" => Style::AlignParentheses,
            _ => Style::Consistent,
        };
        let indentation_width = options
            .get("IndentationWidth")
            .and_then(OptionValue::as_int)
            .or_else(|| {
                options.peer("Layout/IndentationWidth", "Width").and_then(OptionValue::as_int)
            })
            .unwrap_or(2);
        let fixed_indentation_conflict = options
            .peer("Layout/ParameterAlignment", "EnforcedStyle")
            .and_then(OptionValue::as_str)
            == Some("with_fixed_indentation");
        Ok(Self { style, indentation_width, fixed_indentation_conflict })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        let params = def_parameter_list(def.parameters());
        if params.is_empty() {
            return;
        }
        let Some(lparen) = def.lparen_loc() else { return };
        if params.len() >= 2
            && self.style == Style::AlignParentheses
            && self.fixed_indentation_conflict
        {
            return;
        }
        let first = &params[0];
        let lparen_span = lparen.span();
        if ctx.same_line(Span::empty(first.span().start), Span::empty(lparen_span.start)) {
            return;
        }
        self.check_first(ctx, first, lparen_span);
    }
}

impl FirstParameterIndentation {
    /// RuboCop's reduced `indent_base` (see the module doc comment for why
    /// the `:parent_hash_key`/`:first_column_after_left_parenthesis`
    /// branches never apply here).
    fn indent_base(&self, ctx: &Context<'_>, left_paren: Span) -> (i64, BaseType) {
        if self.style == Style::AlignParentheses {
            return (i64::from(ctx.line_col(left_paren.start).column), BaseType::LeftParenthesis);
        }
        let line_text = ctx.line_text(ctx.line_col(left_paren.start).line);
        (i64::from(first_non_ws_column(line_text)), BaseType::StartOfLine)
    }

    /// RuboCop's `check_first`, minus the `detected_styles`/
    /// `ambiguous_style_detected` bookkeeping (used only for
    /// `--auto-gen-config` style inference, which never itself produces an
    /// offense in a single lint run).
    fn check_first(&mut self, ctx: &mut Context<'_>, first: &Node<'_>, left_paren: Span) {
        let first_span = first.span();
        let actual_column = i64::from(ctx.line_col(first_span.start).column);
        let (base_column, base_type) = self.indent_base(ctx, left_paren);
        let expected_column = base_column + self.indentation_width;
        let column_delta = expected_column - actual_column;
        if column_delta == 0 {
            return;
        }
        let msg = message(self.indentation_width, base_type.description());
        let taboo = linter::heredoc_bodies(ctx, first);
        let delta = i32::try_from(column_delta).unwrap_or(0);
        let edits = linter::shift_lines(ctx, first_span, delta, &taboo);
        match fix_from_edits(edits) {
            Some(fix) => ctx.report_with_fix(&Self::META, first_span, msg, fix),
            None => ctx.report(&Self::META, first_span, msg),
        }
    }
}

/// RuboCop's `left_brace.source_line =~ /\S/`: the character column of the
/// first non-whitespace character on the line, or `0` if the line is blank.
fn first_non_ws_column(line: &[u8]) -> u32 {
    match std::str::from_utf8(line) {
        Ok(text) => {
            u32::try_from(text.chars().position(|c| !is_ruby_whitespace_char(c)).unwrap_or(0))
                .unwrap_or(0)
        }
        Err(_) => 0,
    }
}

/// Wraps a possibly-empty edit list from [`linter::shift_lines`] into the
/// `Option<Fix>` shape this cop's call sites report with.
fn fix_from_edits(edits: Vec<Edit>) -> Option<Fix> {
    if edits.is_empty() {
        None
    } else {
        Some(Fix { applicability: Applicability::Safe, edits })
    }
}
