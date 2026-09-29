//! `Style/RescueModifier`, ported from RuboCop's
//! `lib/rubocop/cop/style/rescue_modifier.rb` plus the `Alignment` mixin
//! (`lib/rubocop/cop/mixin/alignment.rb`) and the `ParenthesesCorrector`
//! (`lib/rubocop/cop/correctors/parentheses_corrector.rb`) its
//! autocorrection uses.
//!
//! Prism gives the modifier form its own [`NodeKind::RescueModifierNode`]
//! (`expression rescue rescue_expression`), so there is no whitequark-style
//! `rescue_modifier?` token guard to port (see `duplicate_rescue_exception.rs`'s
//! module doc): every `RescueModifierNode` visited here *is* the offense,
//! directly holding the operation (`expression`) and the handler
//! (`rescue_expression`) upstream reaches via `node.body` and
//! `node.resbody_branches.first.body`.
//!
//! `ParenthesesCorrector`'s handling of a comment above the closing paren, a
//! chained call after it, ternary spacing, and an orphaned trailing comma is
//! not reproduced -- only plain paren removal (consuming adjacent horizontal
//! whitespace after `(` and horizontal/vertical whitespace before `)`, per
//! `RangeHelp#range_with_surrounding_space`) is ported, since no fixture
//! exercises those cases.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Avoid using `rescue` in its modifier form.";

/// Avoid using rescue in its modifier form.
#[derive(Debug, Clone)]
pub struct RescueModifier {
    /// `Alignment#configured_indentation_width`: this cop has no
    /// `IndentationWidth` option of its own, so it always reads
    /// `Layout/IndentationWidth`'s `Width` (default 2).
    indentation_width: i64,
}

impl Rule for RescueModifier {
    const META: RuleMeta = RuleMeta {
        name: "Style/RescueModifier",
        department: Department::Style,
        summary: "Avoid using `rescue` in its modifier form.",
        explanation: "\
The syntax of modifier form `rescue` can be misleading because it might lead \
us to believe that `rescue` handles the given exception but it actually \
rescues all exceptions to return the given rescue block. In this case, value \
returned by handle_error or SomeException.

Modifier form `rescue` would rescue all the exceptions. It would silently \
skip all exceptions or errors and handle the error. Example: If \
`NoMethodError` is raised, modifier form rescue would handle the exception.

```ruby
# bad
some_method rescue handle_error

# bad
some_method rescue SomeException

# good
begin
  some_method
rescue
  handle_error
end

# good
begin
  some_method
rescue SomeException
  handle_error
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::RescueModifierNode],
        config: &[],
        blind_spots: "\
`Layout/IndentationWidth`'s `Width` is read as a peer option (falling back to \
2), matching upstream's `Alignment#configured_indentation_width`. \
`ParenthesesCorrector`'s handling of a comment above the closing paren, a \
chained call after it, ternary spacing, and an orphaned trailing comma is not \
reproduced -- only plain paren removal is ported, since no fixture exercises \
those cases.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .unwrap_or(2);
        Ok(Self { indentation_width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let modifier = node.as_rescue_modifier_node().expect("kind matched");
        let span = node.span();
        let fix = self.build_fix(ctx, &modifier, span);
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}

impl RescueModifier {
    /// RuboCop's `correct_rescue_block` plus `parenthesized?` and
    /// `ParenthesesCorrector.correct`.
    fn build_fix(
        &self,
        ctx: &Context<'_>,
        modifier: &ruby_ast::node::RescueModifierNode<'_>,
        span: Span,
    ) -> Fix {
        let operation = modifier.expression();
        let handler_span = modifier.rescue_expression().span();
        let handler_text = ctx.text(handler_span).to_vec();

        let parens_span = parenthesized_span(ctx);
        let parenthesized = parens_span.is_some();

        let column = usize::try_from(ctx.line_col(span.start).column).unwrap_or(0);
        let width = usize::try_from(self.indentation_width).unwrap_or(2);
        let (offset_len, indent_len) = if parenthesized {
            (column.saturating_sub(1), (column + width).saturating_sub(1))
        } else {
            (column, column + width)
        };
        let offset = " ".repeat(offset_len);
        let indentation = " ".repeat(indent_len);

        let array_wrap = operation.as_array_node().is_some_and(|a| a.opening_loc().is_none());

        let mut edits = Vec::new();

        let mut prefix = format!("begin\n{indentation}");
        if array_wrap {
            prefix.push('[');
        }
        edits.push(Edit::insert(operation.span().start, prefix.into_bytes()));

        let rescue_clause = format!(
            "\n{offset}rescue\n{indentation}{}\n{offset}end",
            String::from_utf8_lossy(&handler_text)
        );

        if let Some(heredoc_end_offset) = heredoc_end(ctx, &operation) {
            let mut removed = String::new();
            if array_wrap {
                removed.push(']');
            }
            edits.push(Edit::replace(
                Span::new(operation.span().end, span.end),
                removed.into_bytes(),
            ));
            edits.push(Edit::insert(heredoc_end_offset, rescue_clause.into_bytes()));
        } else {
            let mut replacement = String::new();
            if array_wrap {
                replacement.push(']');
            }
            replacement.push_str(&rescue_clause);
            edits.push(Edit::replace(
                Span::new(operation.span().end, span.end),
                replacement.into_bytes(),
            ));
        }

        if let Some(parens_span) = parens_span {
            edits.extend(remove_parens(ctx, parens_span));
        }

        Fix { applicability: Applicability::Safe, edits }
    }
}

/// RuboCop's `heredoc_end`: for a call whose last heredoc-shaped argument
/// (searched from the end) is a heredoc, the offset right after that
/// heredoc's terminator identifier -- i.e. Prism's `closing_loc`, trimmed of
/// the trailing whitespace/newline it swallows that whitequark's
/// `heredoc_end` location does not include (see `heredoc_indentation.rs`'s
/// module doc).
fn heredoc_end(ctx: &Context<'_>, node: &Node<'_>) -> Option<u32> {
    let call = node.as_call_node()?;
    let args = call.arguments()?;
    let heredoc =
        args.arguments().iter().collect::<Vec<_>>().into_iter().rev().find(ext::is_heredoc)?;
    let closing = heredoc_closing_span(&heredoc)?;
    Some(trim_trailing_whitespace_end(ctx, closing))
}

/// The `closing_loc` of a heredoc-shaped `StringNode`/`InterpolatedStringNode`/
/// `XStringNode`/`InterpolatedXStringNode`.
fn heredoc_closing_span(node: &Node<'_>) -> Option<Span> {
    match node {
        Node::StringNode { .. } => node.as_string_node()?.closing_loc().map(|l| l.span()),
        Node::InterpolatedStringNode { .. } => {
            node.as_interpolated_string_node()?.closing_loc().map(|l| l.span())
        }
        Node::XStringNode { .. } => Some(node.as_x_string_node()?.closing_loc().span()),
        Node::InterpolatedXStringNode { .. } => {
            Some(node.as_interpolated_x_string_node()?.closing_loc().span())
        }
        _ => None,
    }
}

/// Scans backward from `span.end` over trailing whitespace bytes (` `, `\t`,
/// `\r`, `\n`) to find where the heredoc terminator identifier itself ends.
fn trim_trailing_whitespace_end(ctx: &Context<'_>, span: Span) -> u32 {
    let source = ctx.source().bytes();
    let mut end = span.end as usize;
    while end > span.start as usize && matches!(source[end - 1], b' ' | b'\t' | b'\r' | b'\n') {
        end -= 1;
    }
    u32::try_from(end).expect("offset exceeds u32")
}

/// RuboCop's `parenthesized?`: whether the rescue-modifier node's parent is
/// a paren-grouped expression. Whitequark's `(...)` grouping is a `:begin`
/// node directly, whether it holds one statement or several (elision only
/// applies to a bare, unparenthesized `begin`/`end`); Prism always wraps a
/// `ParenthesesNode`'s body in a `StatementsNode`, one layer deeper, so that
/// wrapper is skipped when present.
fn parenthesized_span(ctx: &Context<'_>) -> Option<Span> {
    let ancestors = ctx.ancestors();
    let parent = ancestors.last()?;
    if parent.kind == NodeKind::ParenthesesNode {
        return Some(parent.span);
    }
    if parent.kind == NodeKind::StatementsNode {
        let grandparent = ancestors.get(ancestors.len() - 2)?;
        if grandparent.kind == NodeKind::ParenthesesNode {
            return Some(grandparent.span);
        }
    }
    None
}

/// RuboCop's `ParenthesesCorrector.correct` for the plain case (no comment
/// above the closing paren, no chained call after it, no ternary, no
/// orphaned comma): removes `(` (and any horizontal whitespace right after
/// it) and `)` (and any whitespace, including newlines, right before it).
fn remove_parens(ctx: &Context<'_>, parens_span: Span) -> Vec<Edit> {
    let source = ctx.source().bytes();

    let mut open_end = parens_span.start + 1;
    while (open_end as usize) < source.len() && matches!(source[open_end as usize], b' ' | b'\t') {
        open_end += 1;
    }

    let mut close_start = parens_span.end - 1;
    while close_start > parens_span.start
        && matches!(source[close_start as usize - 1], b' ' | b'\t' | b'\r' | b'\n')
    {
        close_start -= 1;
    }
    close_start = close_start.max(open_end);

    vec![
        Edit::delete(Span::new(parens_span.start, open_end)),
        Edit::delete(Span::new(close_start, parens_span.end)),
    ]
}
