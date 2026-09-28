//! `Layout/IndentationStyle`, ported from RuboCop's
//! `lib/rubocop/cop/layout/indentation_style.rb` plus the amount of the
//! `Alignment` mixin it uses (`configured_indentation_width`).
//!
//! Upstream scans `processed_source.lines` directly (a raw-text scan, not a
//! node walk) looking for a leading run of tabs (`EnforcedStyle: spaces`) or
//! a leading run of spaces (`EnforcedStyle: tabs`) at the very start of each
//! line, then excludes any such match that starts inside a plain string
//! literal or heredoc body (`in_string_literal?`/`string_literal_ranges`,
//! built from every `:str`/`:dstr` node). This only needs [`NodeKind::StringNode`]/
//! [`NodeKind::InterpolatedStringNode`] subscriptions to collect those
//! excluded ranges as the tree is walked; the line scan itself runs once at
//! [`Rule::file_end`], mirroring [`crate::layout::trailing_whitespace`]'s
//! `__END__`-aware line iteration.
//!
//! Deliberately *not* excluded: comment lines (RuboCop's `:str`/`:dstr`
//! check does not cover `Comment` nodes) and `xstr`/regexp/symbol literals
//! (upstream's `each_node(:str, :dstr)` skips them too) -- so those raw-text
//! bytes are compared against, not skipped, the same as upstream.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Spaces,
    Tabs,
}

/// Checks that the indentation method is consistent: either tabs only or
/// spaces only are used for indentation.
#[derive(Debug, Clone)]
pub struct IndentationStyle {
    style: Style,
    /// RuboCop's `Alignment#configured_indentation_width`: this cop's own
    /// `IndentationWidth`, else `Layout/IndentationWidth`'s `Width`, else 2.
    indentation_width: i64,
    /// Precomputed from `style` (RuboCop's `message`).
    message: &'static str,
    /// `(first_line, end_line)` of every heredoc body found so far this
    /// file, `end_line` exclusive (RuboCop's `loc.heredoc_body`).
    heredoc_lines: Vec<(u32, u32)>,
    /// Full span of every non-heredoc quoted string found so far this file
    /// (RuboCop's `loc.expression`, gated on `loc?(:begin)`).
    quoted_spans: Vec<Span>,
}

impl IndentationStyle {
    /// RuboCop's `string_literal_ranges`' per-node contribution: a heredoc
    /// contributes its body's line range, any other delimited string
    /// literal its whole span. A string with no delimiters at all (e.g. one
    /// part of a bare `%w[]`/`%W[]` array) contributes nothing, matching
    /// `str.loc?(:begin)` being false there.
    fn record_string(
        &mut self,
        ctx: &Context<'_>,
        node: &Node<'_>,
        opening: Option<Span>,
        closing: Option<Span>,
    ) {
        let (Some(opening), Some(closing)) = (opening, closing) else { return };
        if ctx.text(opening).starts_with(b"<<") {
            let first_line = ctx.line_col(opening.start).line + 1;
            let end_line = ctx.line_col(closing.start).line;
            self.heredoc_lines.push((first_line, end_line));
        } else {
            self.quoted_spans.push(node.span());
        }
    }

    /// RuboCop's `in_string_literal?`: `range` (always a line-initial span)
    /// falls inside a previously recorded heredoc body or quoted string.
    fn in_string_literal(&self, line: u32, range: Span) -> bool {
        self.heredoc_lines.iter().any(|&(first, end)| line >= first && line < end)
            || self.quoted_spans.iter().any(|q| q.start <= range.start && range.end <= q.end)
    }

    /// RuboCop's `autocorrect`: dispatches on which whitespace character the
    /// matched range actually contains.
    fn autocorrect(&self, ctx: &Context<'_>, range: Span) -> Fix {
        let text = ctx.text(range);
        let replacement = match self.style {
            // `autocorrect_lambda_for_tabs`: every tab becomes `width`
            // spaces; other bytes (leading spaces before the last tab)
            // pass through unchanged.
            Style::Spaces => {
                let width = usize::try_from(self.indentation_width.max(0)).unwrap_or(0);
                let mut out = Vec::with_capacity(text.len());
                for &byte in text {
                    if byte == b'\t' {
                        out.resize(out.len() + width, b' ');
                    } else {
                        out.push(byte);
                    }
                }
                out
            }
            // `autocorrect_lambda_for_spaces`: the whole (all-whitespace)
            // match is replaced by `size / width` tabs, rounding down.
            Style::Tabs => {
                let width = usize::try_from(self.indentation_width.max(1)).unwrap_or(1);
                vec![b'\t'; text.len() / width]
            }
        };
        Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(range, replacement)] }
    }
}

/// RuboCop's `find_offense`: the byte length of the leading run of
/// whitespace up to and including its rightmost tab (`EnforcedStyle:
/// spaces`) or rightmost space (`EnforcedStyle: tabs`), or `None` when the
/// leading run has no such character. Equivalent to matching
/// `/\A\s*\t+/`/`/\A\s* +/` and taking `match.end(0)`: the greedy `\s*`
/// backtracks only until the immediately following character can start the
/// required run, which is exactly the rightmost occurrence of the target
/// character within the leading whitespace.
fn find_offense(line: &[u8], style: Style) -> Option<u32> {
    let mut end = 0usize;
    while end < line.len() && matches!(line[end], b' ' | b'\t') {
        end += 1;
    }
    let target = match style {
        Style::Spaces => b'\t',
        Style::Tabs => b' ',
    };
    let last = line[..end].iter().rposition(|&b| b == target)?;
    u32::try_from(last + 1).ok()
}

impl Rule for IndentationStyle {
    const META: RuleMeta = RuleMeta {
        name: "Layout/IndentationStyle",
        department: Department::Layout,
        summary: "Checks that the indentation method is consistent.",
        explanation: "\
Either tabs only or spaces only are used for indentation.

```ruby
# EnforcedStyle: spaces (default)
# bad
# This example uses a tab to indent bar.
def foo
\tbar
end

# good
# This example uses spaces to indent bar.
def foo
  bar
end
```

```ruby
# EnforcedStyle: tabs
# bad
# This example uses spaces to indent bar.
def foo
  bar
end

# good
# This example uses a tab to indent bar.
def foo
\tbar
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StringNode, NodeKind::InterpolatedStringNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("spaces"),
                allowed: &["spaces", "tabs"],
                doc: "Which whitespace character indentation must consist of.",
            },
            ConfigOption {
                name: "IndentationWidth",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "\
Number of spaces a tab is replaced by (or that make up a tab) during \
autocorrection. Defaults to `Layout/IndentationWidth`'s `Width`.",
            },
        ],
        blind_spots: "\
The line scan runs over every line up to (but not including) a trailing
`__END__` data section, and is not otherwise aware of node boundaries: a
comment line, or the first line of a multi-line string literal (before its
opening delimiter), is compared exactly as upstream does, since neither is
a `:str`/`:dstr` range.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "tabs" => Style::Tabs,
            _ => Style::Spaces,
        };
        let indentation_width = options
            .get("IndentationWidth")
            .and_then(OptionValue::as_int)
            .or_else(|| {
                options.peer("Layout/IndentationWidth", "Width").and_then(OptionValue::as_int)
            })
            .unwrap_or(2);
        let message = match style {
            Style::Spaces => "Tab detected in indentation.",
            Style::Tabs => "Space detected in indentation.",
        };
        Ok(Self {
            style,
            indentation_width,
            message,
            heredoc_lines: Vec::new(),
            quoted_spans: Vec::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.heredoc_lines.clear();
        self.quoted_spans.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::StringNode => {
                let n = node.as_string_node().expect("kind matched");
                let opening = n.opening_loc().map(|l| l.span());
                let closing = n.closing_loc().map(|l| l.span());
                self.record_string(ctx, node, opening, closing);
            }
            NodeKind::InterpolatedStringNode => {
                let n = node.as_interpolated_string_node().expect("kind matched");
                let opening = n.opening_loc().map(|l| l.span());
                let closing = n.closing_loc().map(|l| l.span());
                self.record_string(ctx, node, opening, closing);
            }
            _ => {}
        }
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        // RuboCop inspects `processed_source.lines`, which stops at a
        // `__END__` data section.
        let last_line = match ctx.parsed().data_span() {
            Some(data) => ctx.line_col(data.start).line.saturating_sub(1),
            None => ctx.line_count(),
        };

        for (line, span) in ctx.lines().take_while(|&(line, _)| line <= last_line) {
            let Some(len) = find_offense(ctx.text(span), self.style) else { continue };
            let range = Span::new(span.start, span.start + len);
            if self.in_string_literal(line, range) {
                continue;
            }
            let fix = self.autocorrect(ctx, range);
            ctx.report_with_fix(&Self::META, range, self.message, fix);
        }
    }
}
