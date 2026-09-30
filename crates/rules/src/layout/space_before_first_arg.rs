//! `Layout/SpaceBeforeFirstArg`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_before_first_arg.rb` plus the
//! `PrecedingFollowingAlignment` mixin
//! (`lib/rubocop/cop/mixin/preceding_following_alignment.rb`) it includes for
//! its `AllowForAlignment` option.
//!
//! Prism folds a literal `&block`/`&:sym` pass into `CallNode::block` (a
//! `BlockArgumentNode`), unlike whitequark's `send` node, which keeps it as
//! an ordinary trailing argument counted by `arguments?`/`first_argument`;
//! [`effective_first_argument`] falls back to that block-pass node when the
//! regular argument list is empty (`foo &blk`), matching whitequark's shape.
//!
//! `AllowForAlignment`'s alignment search
//! ([`aligned_with_something`]/[`aligned_with_line`]/[`check_line_alignment`])
//! reimplements the mixin's `aligned_with_adjacent_line?`/`aligned_token?`
//! walk directly against source text via [`Context::line_text`], since there
//! is no real token stream to consult (see `META.blind_spots`).

use std::collections::HashSet;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{is_ruby_whitespace, Side, Span};

use crate::layout::first_argument_indentation::is_operator_method;

/// RuboCop's `MSG`.
const MSG: &str = "Put one space between the method name and the first argument.";

/// Checks that exactly one space is used between a method name and the first argument for
/// method calls without parentheses.
#[derive(Debug, Clone)]
pub struct SpaceBeforeFirstArg {
    allow_for_alignment: bool,
}

impl Rule for SpaceBeforeFirstArg {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceBeforeFirstArg",
        department: Department::Layout,
        summary: "Checks that exactly one space is used between a method name and the first \
                   argument for method calls without parentheses.",
        explanation: "\
```ruby
# bad
something  x
something   y, z
something'hello'

# good
something x
something y, z
something 'hello'
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[linter::ConfigOption {
            name: "AllowForAlignment",
            default: linter::ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Allow extra spacing that lines up the first argument with the previous or \
                  next line.",
        }],
        blind_spots: "\
`AllowForAlignment`'s alignment search only matches RuboCop's own `aligned_words?` half of
`aligned_token?` (an exact-text or space-then-non-space column match on another line);
`aligned_equals_operator?`'s equals-sign fallback is not reproduced, since it requires the
checked range's own source text to end in `=`, which a method call's first argument
practically never does. Column comparisons index by byte offset within a line, i.e. assume
one byte per character; a line with multi-byte UTF-8 content before the compared column can
misalign the comparison.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_for_alignment: options.bool("AllowForAlignment") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let NodeKind::CallNode = node.kind() else { return };
        let call = node.as_call_node().expect("kind matched");
        self.check(&call, ctx);
    }
}

impl SpaceBeforeFirstArg {
    /// RuboCop's `on_send`/`on_csend` (Prism unifies both under `CallNode`).
    fn check(&self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        if call.equal_loc().is_some() {
            return; // setter_method?
        }
        if is_operator_method(call.name().as_slice()) {
            return;
        }
        if call.closing_loc().is_some() {
            return; // parenthesized?
        }
        let Some(first_arg_span) = effective_first_argument(call) else { return };
        let Some(message_span) = call.message_loc().map(|l| l.span()) else { return };

        let space_start = ctx.with_surrounding_space(first_arg_span, Side::Left, true, false).start;
        let space = Span::new(space_start, first_arg_span.start);
        if space.end - space.start == 1 {
            return;
        }

        if !self.expect_params_after_method_name(
            ctx,
            call.location().span(),
            message_span,
            first_arg_span,
        ) {
            return;
        }

        ctx.report_with_fix(
            &Self::META,
            space,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(space, *b" ")] },
        );
    }

    /// RuboCop's `expect_params_after_method_name?`.
    fn expect_params_after_method_name(
        &self,
        ctx: &Context<'_>,
        call_span: Span,
        message_span: Span,
        first_arg_span: Span,
    ) -> bool {
        if message_span.end == first_arg_span.start {
            return true; // no_space_between_method_name_and_first_argument?
        }
        if !ctx.same_line(call_span, first_arg_span) {
            return false;
        }
        !(self.allow_for_alignment && aligned_with_something(ctx, first_arg_span))
    }
}

/// RuboCop-AST's `first_argument`/`arguments?`, extended to fall back to a `&block` pass when
/// there are no regular arguments (see the module docs).
fn effective_first_argument(call: &CallNode<'_>) -> Option<Span> {
    if let Some(first) = call.arguments().and_then(|a| a.arguments().first()) {
        return Some(first.span());
    }
    call.block().and_then(|b| b.as_block_argument_node()).map(|b| b.as_node().span())
}

/// RuboCop's `aligned_with_something?`: does `span` (the first argument's own source range) line
/// up with something meaningful on a preceding or following line? Mirrors
/// `aligned_with_adjacent_line?` exactly: each of the "lines before" / "lines after" candidate
/// lists is walked nearest-line first, stopping at (and deciding by) the first line that isn't
/// blank and isn't a standalone comment. Only if neither direction's nearest candidate matches
/// does a second pass retry both directions, this time additionally requiring the candidate's
/// indentation column to equal `span`'s own line's indentation (RuboCop's `base_indentation`
/// fallback).
fn aligned_with_something(ctx: &Context<'_>, span: Span) -> bool {
    let line = ctx.line_col(span.start).line;
    let line_span = ctx.line_span(line);
    let col = usize::try_from(span.start - line_span.start).unwrap_or(usize::MAX);
    let token_text = ctx.text(span);
    let standalone_comment_lines: HashSet<u32> =
        ctx.comments().iter().filter(|c| ctx.begins_its_line(c.span)).map(|c| c.line).collect();
    let count = ctx.line_count();

    if aligned_with_line(ctx, (1..line).rev(), None, col, token_text, &standalone_comment_lines)
        || aligned_with_line(
            ctx,
            (line + 1)..=count,
            None,
            col,
            token_text,
            &standalone_comment_lines,
        )
    {
        return true;
    }

    let base_indentation = line_indentation(ctx, line);
    aligned_with_line(
        ctx,
        (1..line).rev(),
        Some(base_indentation),
        col,
        token_text,
        &standalone_comment_lines,
    ) || aligned_with_line(
        ctx,
        (line + 1)..=count,
        Some(base_indentation),
        col,
        token_text,
        &standalone_comment_lines,
    )
}

/// RuboCop's `aligned_with_line?`: scans `line_nos` (already ordered nearest candidate first) for
/// the first line that is neither blank nor a standalone comment -- and, once a required `indent`
/// is given, also skips lines whose own indentation column doesn't match it -- then returns
/// [`check_line_alignment`]'s verdict on just that single line.
fn aligned_with_line(
    ctx: &Context<'_>,
    line_nos: impl Iterator<Item = u32>,
    indent: Option<usize>,
    col: usize,
    token_text: &[u8],
    standalone_comment_lines: &HashSet<u32>,
) -> bool {
    for candidate in line_nos {
        if standalone_comment_lines.contains(&candidate) {
            continue;
        }
        let line_bytes = ctx.line_text(candidate);
        if line_bytes.iter().all(|&b| is_ruby_whitespace(b)) {
            continue;
        }
        if let Some(want) = indent {
            if line_indentation(ctx, candidate) != want {
                continue;
            }
        }
        return check_line_alignment(ctx, candidate, col, token_text);
    }
    false
}

/// RuboCop's `aligned_words?` (see `META.blind_spots` for why `aligned_equals_operator?` is not
/// also reproduced here). The caller ([`aligned_with_line`]) has already established `candidate`
/// is non-blank and not a standalone comment.
fn check_line_alignment(ctx: &Context<'_>, candidate: u32, col: usize, token_text: &[u8]) -> bool {
    let line_bytes = ctx.line_text(candidate);
    if col >= 1 {
        if let (Some(&before), Some(&at)) = (line_bytes.get(col - 1), line_bytes.get(col)) {
            if before == b' ' && at != b' ' {
                return true;
            }
        }
    }
    !token_text.is_empty() && line_bytes.get(col..col + token_text.len()) == Some(token_text)
}

/// RuboCop's `ProcessedSource#line_indentation`: the character count of the run of `\s` bytes at
/// the start of `line`, expressed as a display column (equal to the character count, since `\s`
/// bytes are always width 1).
fn line_indentation(ctx: &Context<'_>, line: u32) -> usize {
    let text = ctx.line_text(line);
    let offset = text.iter().take_while(|&&b| is_ruby_whitespace(b)).count();
    let line_start = ctx.line_span(line).start;
    let column = ctx.display_column(line_start + u32::try_from(offset).unwrap_or(u32::MAX));
    usize::try_from(column).unwrap_or(usize::MAX)
}
