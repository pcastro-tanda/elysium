//! `Layout/EmptyLines`, ported from RuboCop's `lib/rubocop/cop/layout/empty_lines.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Extra blank line detected.";
/// RuboCop's `LINE_OFFSET`.
const LINE_OFFSET: u32 = 2;

/// Checks for two or more consecutive blank lines.
///
/// RuboCop drives this from `processed_source.tokens`: a blank line only
/// counts as "extra" when no token starts on it. Multi-line string, x-string
/// and regexp literals get one content token per physical line (even blank
/// ones) from the `parser` gem's lexer, so blank lines inside them are never
/// flagged; a heredoc's declaration line carries its own token but its body
/// (recorded separately below, mirroring `Layout::TrailingWhitespace`) does
/// too. Every other blank line - including inside non-literal multi-line
/// constructs such as method calls or array literals - has no token and is a
/// candidate.
#[derive(Debug, Clone, Default)]
pub struct EmptyLines {
    /// Physical line ranges `(first_line, end_line)` (end exclusive) covered
    /// by a multi-line string/heredoc/regexp literal, in traversal order.
    literal_lines: Vec<(u32, u32)>,
}

impl EmptyLines {
    /// Marks lines `first..=last` as carrying a synthetic per-line token.
    fn mark(&mut self, first_line: u32, last_line: u32) {
        if last_line > first_line {
            self.literal_lines.push((first_line, last_line + 1));
        }
    }

    /// Records a `String`/`XString`-family node. A heredoc (detected by its
    /// opening delimiter starting with `<<`) is marked over its body only -
    /// from the line after the declaration through the line before the
    /// closing delimiter, matching `Layout::TrailingWhitespace`'s heredoc
    /// handling, since the node's own location covers just the declaration
    /// token. Any other multi-line literal is marked over its own span.
    fn record_string(
        &mut self,
        ctx: &Context<'_>,
        node_span: Span,
        opening: Option<Span>,
        closing: Option<Span>,
    ) {
        if let (Some(open), Some(close)) = (opening, closing) {
            if ctx.text(open).starts_with(b"<<") {
                self.mark(ctx.line_col(open.start).line + 1, ctx.line_col(close.start).line);
                return;
            }
        }
        self.mark(ctx.line_col(node_span.start).line, ctx.line_col(node_span.end).line);
    }

    /// True when `line` falls inside a recorded literal's line range.
    fn is_literal_line(&self, line: u32) -> bool {
        self.literal_lines.iter().any(|&(first, end)| line >= first && line < end)
    }
}

impl Rule for EmptyLines {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLines",
        department: Department::Layout,
        summary: "Checks for two or more consecutive blank lines.",
        explanation: "\
```ruby
# bad - it has two empty lines.
some_method
# one empty line
# two empty lines
some_method

# good
some_method
# one empty line
some_method
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::StringNode,
            NodeKind::InterpolatedStringNode,
            NodeKind::XStringNode,
            NodeKind::InterpolatedXStringNode,
            NodeKind::RegularExpressionNode,
            NodeKind::InterpolatedRegularExpressionNode,
        ],
        config: &[],
        blind_spots: "\
Blank lines are never flagged inside a multi-line string, x-string or
regexp literal (matching RuboCop's per-line lexer tokens for those). The
same per-line-token behaviour also exempts `%w`/`%i` word/symbol arrays and
plain arrays/method calls from ever gaining a synthetic token for a blank
line, which this port already treats correctly by leaving them untouched -
their blank lines are ordinary candidates, exactly as in real RuboCop.
Percent-literal arrays (`%w`, `%i`, `%W`, `%I`) and other multi-line
constructs are not modelled as literal spans because RuboCop's own lexer
does not special-case them either.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.literal_lines.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::StringNode { .. } => {
                let n = node.as_string_node().expect("kind matched");
                let span = n.location().span();
                let opening = n.opening_loc().map(|l| l.span());
                let closing = n.closing_loc().map(|l| l.span());
                self.record_string(ctx, span, opening, closing);
            }
            Node::InterpolatedStringNode { .. } => {
                let n = node.as_interpolated_string_node().expect("kind matched");
                let span = n.location().span();
                let opening = n.opening_loc().map(|l| l.span());
                let closing = n.closing_loc().map(|l| l.span());
                self.record_string(ctx, span, opening, closing);
            }
            Node::XStringNode { .. } => {
                let n = node.as_x_string_node().expect("kind matched");
                let span = n.location().span();
                self.record_string(
                    ctx,
                    span,
                    Some(n.opening_loc().span()),
                    Some(n.closing_loc().span()),
                );
            }
            Node::InterpolatedXStringNode { .. } => {
                let n = node.as_interpolated_x_string_node().expect("kind matched");
                let span = n.location().span();
                self.record_string(
                    ctx,
                    span,
                    Some(n.opening_loc().span()),
                    Some(n.closing_loc().span()),
                );
            }
            Node::RegularExpressionNode { .. } => {
                let n = node.as_regular_expression_node().expect("kind matched");
                let span = n.location().span();
                self.mark(ctx.line_col(span.start).line, ctx.line_col(span.end).line);
            }
            Node::InterpolatedRegularExpressionNode { .. } => {
                let n = node.as_interpolated_regular_expression_node().expect("kind matched");
                let span = n.location().span();
                self.mark(ctx.line_col(span.start).line, ctx.line_col(span.end).line);
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

        // RuboCop's `each_extra_empty_line`: walk the lines that carry a
        // token, and for each gap wider than `LINE_OFFSET` re-scan its
        // interior for runs of two exactly-empty lines.
        let mut prev_line: u32 = 1;
        for cur_line in 1..=last_line {
            let cur_has_token =
                !is_blank_ish(ctx.line_text(cur_line)) || self.is_literal_line(cur_line);
            if !cur_has_token {
                continue;
            }
            if cur_line - prev_line > LINE_OFFSET {
                for line in (prev_line + 1)..cur_line {
                    let empty =
                        ctx.line_text(line - 1).is_empty() && ctx.line_text(line).is_empty();
                    if empty {
                        let line_span = ctx.line_span(line);
                        let range = Span::new(line_span.start, line_span.start + 1);
                        ctx.report_with_fix(
                            &Self::META,
                            range,
                            MSG,
                            Fix {
                                applicability: Applicability::Safe,
                                edits: vec![Edit::delete(range)],
                            },
                        );
                    }
                }
            }
            prev_line = cur_line;
        }
    }
}

/// A line the real lexer would skip entirely (no token starts on it):
/// nothing but horizontal whitespace, including none at all.
fn is_blank_ish(text: &[u8]) -> bool {
    text.iter().all(|&b| b == b' ' || b == b'\t')
}
