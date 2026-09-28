//! `Naming/HeredocDelimiterCase`, ported from RuboCop's
//! `lib/rubocop/cop/naming/heredoc_delimiter_case.rb` plus the `Heredoc`
//! mixin it includes.
//!
//! # Locations
//!
//! Whitequark's `node.source_range` (aliased as upstream's local `expr`) for
//! a heredoc-shaped `str`/`dstr`/`xstr` node is just its opening line (e.g.
//! `<<-'sql'`), never the body or closing delimiter -- Prism's `opening_loc`
//! matches that directly for every heredoc-shaped
//! `StringNode`/`InterpolatedStringNode`/`XStringNode`/
//! `InterpolatedXStringNode`. Whitequark's `node.loc.heredoc_end` is the
//! closing delimiter's own physical line, leading indentation included but
//! its trailing line terminator excluded (confirmed against real RuboCop:
//! an indented `<<-sql`'s offense/autocorrect range covers `  sql`, not just
//! `sql`) -- exactly [`Context::line_span`] of the line Prism's `closing_loc`
//! starts on, since that always starts at the physical line's own first
//! byte too.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::{ext, LocationExt as _, Node, NodeKind};
use ruby_source::Span;
use std::sync::LazyLock;

/// RuboCop's `Heredoc::OPENING_DELIMITER`.
static OPENING_DELIMITER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(<<[~-]?)['"`]?([^'"`]+)['"`]?"#).expect("valid regex"));

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Uppercase,
    Lowercase,
}

impl Style {
    /// The style name as it appears in `MSG`.
    fn name(self) -> &'static str {
        match self {
            Self::Uppercase => "uppercase",
            Self::Lowercase => "lowercase",
        }
    }

    /// RuboCop's `correct_delimiters`.
    fn correct(self, source: &[u8]) -> Vec<u8> {
        match self {
            Self::Uppercase => source.to_ascii_uppercase(),
            Self::Lowercase => source.to_ascii_lowercase(),
        }
    }
}

/// Checks that your heredocs are using the configured case. By default it
/// is configured to enforce uppercase heredocs.
#[derive(Debug, Clone)]
pub struct HeredocDelimiterCase {
    style: Style,
}

impl HeredocDelimiterCase {
    /// The heredoc's own `opening`/`closing` locations, for any
    /// heredoc-shaped `StringNode`/`InterpolatedStringNode`/`XStringNode`/
    /// `InterpolatedXStringNode`; `None` for anything else (or a heredoc
    /// missing a location, which cannot happen but Prism's API is
    /// `Option`-typed).
    fn heredoc_locs(node: &Node<'_>) -> Option<(Span, Span)> {
        if !ext::is_heredoc(node) {
            return None;
        }
        Some(match node {
            Node::StringNode { .. } => {
                let n = node.as_string_node().expect("kind matched");
                let (Some(o), Some(c)) = (n.opening_loc(), n.closing_loc()) else { return None };
                (o.span(), c.span())
            }
            Node::InterpolatedStringNode { .. } => {
                let n = node.as_interpolated_string_node().expect("kind matched");
                let (Some(o), Some(c)) = (n.opening_loc(), n.closing_loc()) else { return None };
                (o.span(), c.span())
            }
            Node::XStringNode { .. } => {
                let n = node.as_x_string_node().expect("kind matched");
                (n.opening_loc().span(), n.closing_loc().span())
            }
            Node::InterpolatedXStringNode { .. } => {
                let n = node.as_interpolated_x_string_node().expect("kind matched");
                (n.opening_loc().span(), n.closing_loc().span())
            }
            _ => return None,
        })
    }

    /// RuboCop's `Heredoc#delimiter_string`, applied directly to the
    /// opening delimiter's own source text -- whitequark's `node.source`
    /// (matched against the same regex in `correct_case_delimiters?`) is
    /// just that for a heredoc-shaped node, which Prism's `opening_loc`
    /// text matches verbatim. Returns the delimiter identifier with its
    /// surrounding quote (if any) stripped, e.g. `sql` out of `<<-'sql'`.
    fn delimiter_string(opening_source: &[u8]) -> Option<&[u8]> {
        let text = std::str::from_utf8(opening_source).ok()?;
        let m = OPENING_DELIMITER.captures(text)?.get(2)?;
        Some(&opening_source[m.start()..m.end()])
    }

    /// RuboCop's `on_heredoc` plus `correct_case_delimiters?`.
    fn check_heredoc(&self, ctx: &mut Context<'_>, opening: Span, closing: Span) {
        let opening_source = ctx.text(opening);
        let Some(delimiter) = Self::delimiter_string(opening_source) else { return };
        let corrected_delimiter = self.style.correct(delimiter);
        if delimiter == corrected_delimiter.as_slice() {
            return;
        }

        let closing_line = ctx.line_col(closing.start).line;
        let heredoc_end = ctx.line_span(closing_line);
        let message = format!("Use {} heredoc delimiters.", self.style.name());

        let edits = vec![
            Edit::replace(opening, self.style.correct(opening_source)),
            Edit::replace(heredoc_end, corrected_delimiter),
        ];
        ctx.report_with_fix(
            &Self::META,
            heredoc_end,
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

impl Rule for HeredocDelimiterCase {
    const META: RuleMeta = RuleMeta {
        name: "Naming/HeredocDelimiterCase",
        department: Department::Naming,
        summary: "Checks that your heredocs are using the configured case.",
        explanation: "\
By default it is configured to enforce uppercase heredocs.

```ruby
# EnforcedStyle: uppercase (default)
# bad
<<-sql
  SELECT * FROM foo
sql

# good
<<-SQL
  SELECT * FROM foo
SQL
```

```ruby
# EnforcedStyle: lowercase
# bad
<<-SQL
  SELECT * FROM foo
SQL

# good
<<-sql
  SELECT * FROM foo
sql
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::StringNode,
            NodeKind::InterpolatedStringNode,
            NodeKind::XStringNode,
            NodeKind::InterpolatedXStringNode,
        ],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("uppercase"),
            allowed: &["uppercase", "lowercase"],
            doc: "Whether heredoc delimiters should be uppercase or lowercase.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "lowercase" => Style::Lowercase,
            _ => Style::Uppercase,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some((opening, closing)) = Self::heredoc_locs(node) else { return };
        self.check_heredoc(ctx, opening, closing);
    }
}
