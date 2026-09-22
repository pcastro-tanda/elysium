//! Shared logic for `Style/TrailingCommaInArguments`, `Style/TrailingCommaInArrayLiteral`, and
//! `Style/TrailingCommaInHashLiteral`, ported from RuboCop's `TrailingComma` mixin
//! (`lib/rubocop/cop/mixin/trailing_comma.rb`).
//!
//! Each cop owns extracting its own node-specific inputs (which items/elements to check, whether
//! any of them is a heredoc, whether the last one is a block-pass argument) into a
//! [`TrailingCommaNode`] and calls [`check`] with it; this module implements the style-independent
//! comma search, the `multiline?`/`should_have_comma?` family, and the fix/message construction
//! that all three cops share.

use linter::{Applicability, Context, Edit, Fix, RuleMeta};
use ruby_ast::{LocationExt as _, Node};
use ruby_source::Span;

/// RuboCop's `EnforcedStyleForMultiline` (shared `style_parameter_name`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultilineStyle {
    /// Comma required only when every item is on its own line.
    Comma,
    /// Comma required for every multiline call/literal, regardless of item layout.
    ConsistentComma,
    /// Comma required only when the last item immediately precedes a newline.
    DiffComma,
    /// Comma never required (but still forbidden on single-line calls/literals).
    NoComma,
}

impl MultilineStyle {
    /// Parses a `EnforcedStyleForMultiline`/`SupportedStylesForMultiline` value. Unknown values
    /// (already rejected by `RuleOptions::style`) fall back to `no_comma`.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "comma" => Self::Comma,
            "consistent_comma" => Self::ConsistentComma,
            "diff_comma" => Self::DiffComma,
            _ => Self::NoComma,
        }
    }
}

/// One call/literal to check for a required/forbidden trailing comma. Each including cop
/// populates this from its own node type; see field docs for the RuboCop equivalent.
pub struct TrailingCommaNode {
    /// RuboCop's `elements(node)`: spans used only to test multiline-ness
    /// (`no_elements_on_same_line?`, `allowed_multiline_argument?`). For method calls, a multiline
    /// braceless-hash argument is flattened into its pairs; array/hash literals pass their
    /// children as-is.
    pub elements: Vec<Span>,
    /// The checked construct's own span used for `node.multiline?`/`node.last_line`: for method
    /// calls, from the receiver/message through the closing paren/bracket (excluding any attached
    /// block, which RuboCop's `send`/`csend` node never includes either); for array/hash literals,
    /// the whole literal.
    pub node_span: Span,
    /// The closing bracket/brace/paren token (RuboCop's `node.loc.end`).
    pub closing: Span,
    /// The last raw item's span (RuboCop's `items.last`/`node.last_argument`/
    /// `node.children.last`): the trailing-comma search start and the autocorrect anchor.
    pub last_item: Span,
    /// True when the last raw item is a block-pass (`&blk`); RuboCop's `put_comma` never inserts a
    /// comma there (`&blk,` is invalid Ruby).
    pub last_item_is_block_pass: bool,
    /// True when any raw item is (or, for a call/hash pair, resolves through to) a heredoc -
    /// relaxes the trailing-comma search to stop at the first newline instead of crossing into the
    /// heredoc body. See [`is_heredoc`].
    pub any_heredoc: bool,
    /// `Some(line)` of the method's selector (RuboCop's `node.loc.selector&.line || node.loc.line`)
    /// for method calls - used by `method_name_and_arguments_on_same_line?`; `None` for array/hash
    /// literals, where `node.call_type?` is false and that check is unconditionally false.
    pub selector_line: Option<u32>,
    /// True when the last raw item is a braced hash literal (RuboCop's `node.last_argument.hash_type?
    /// && braces?`) - short-circuits `method_name_and_arguments_on_same_line?` to true.
    pub last_is_braced_hash: bool,
}

/// Ruby's `\s` (POSIX): space, tab, newline, CR, vertical tab, form feed.
const fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

/// `[^\S\n]`: `\s` minus the newline - used for the heredoc-aware comma search so it never reads
/// past the tag's own line into the body.
const fn is_space_not_newline(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | 0x0B | 0x0C)
}

fn first_line(ctx: &Context<'_>, span: Span) -> u32 {
    ctx.line_col(span.start).line
}

/// RuboCop's `Range#last_line`: the line containing the range's last byte.
fn last_line(ctx: &Context<'_>, span: Span) -> u32 {
    let end = span.end.saturating_sub(1).max(span.start);
    ctx.line_col(end).line
}

/// RuboCop's `Node#multiline?` on an arbitrary span: does it cross a line boundary.
#[must_use]
pub fn is_multiline_span(ctx: &Context<'_>, span: Span) -> bool {
    first_line(ctx, span) != last_line(ctx, span)
}

/// RuboCop's `Util.begins_its_line?`: true when only blank characters precede `span` on its line.
fn begins_its_line(ctx: &Context<'_>, span: Span) -> bool {
    let line = ctx.line_col(span.start).line;
    let line_start = ctx.line_span(line).start;
    ctx.text(Span::new(line_start, span.start)).iter().all(|&b| b == b' ' || b == b'\t')
}

/// RuboCop's `allowed_multiline_argument?`: a single element whose closing bracket does not begin
/// its own line is not considered multiline, even if the element itself spans several lines (e.g.
/// a single heredoc argument whose closing paren sits right after the heredoc tag).
fn allowed_multiline_argument(ctx: &Context<'_>, node: &TrailingCommaNode) -> bool {
    node.elements.len() == 1 && !begins_its_line(ctx, node.closing)
}

/// RuboCop's `multiline?`.
fn is_multiline(ctx: &Context<'_>, node: &TrailingCommaNode) -> bool {
    is_multiline_span(ctx, node.node_span) && !allowed_multiline_argument(ctx, node)
}

/// RuboCop's `no_elements_on_same_line?`: none of the elements, followed by the closing token, may
/// start on the line the previous one ended on.
fn no_elements_on_same_line(ctx: &Context<'_>, node: &TrailingCommaNode) -> bool {
    let mut prev_last_line: Option<u32> = None;
    for &el in node.elements.iter().chain(std::iter::once(&node.closing)) {
        if let Some(prev) = prev_last_line {
            if prev == first_line(ctx, el) {
                return false;
            }
        }
        prev_last_line = Some(last_line(ctx, el));
    }
    true
}

/// RuboCop's `method_name_and_arguments_on_same_line?`.
fn method_name_and_arguments_on_same_line(ctx: &Context<'_>, node: &TrailingCommaNode) -> bool {
    let Some(selector_line) = node.selector_line else { return false };
    if last_line(ctx, node.node_span) != last_line(ctx, node.last_item) {
        return false;
    }
    if node.last_is_braced_hash {
        return true;
    }
    selector_line == last_line(ctx, node.last_item)
}

/// RuboCop's `last_item_precedes_newline?`: does the text right after the last item (skipping an
/// optional comma, whitespace, and at most one trailing `#...` comment) reach a newline before any
/// other content.
fn last_item_precedes_newline(ctx: &Context<'_>, node: &TrailingCommaNode) -> bool {
    let text = ctx.text(Span::new(node.last_item.end, node.node_span.end));
    let mut i = 0;
    if text.first() == Some(&b',') {
        i = 1;
    }
    loop {
        match text.get(i) {
            Some(b'\n') => return true,
            Some(&b) if is_space(b) => i += 1,
            Some(b'#') => {
                while matches!(text.get(i), Some(&b) if b != b'\n') {
                    i += 1;
                }
            }
            _ => return false,
        }
    }
}

/// RuboCop's `should_have_comma?`.
fn should_have_comma(ctx: &Context<'_>, style: MultilineStyle, node: &TrailingCommaNode) -> bool {
    match style {
        MultilineStyle::Comma => is_multiline(ctx, node) && no_elements_on_same_line(ctx, node),
        MultilineStyle::ConsistentComma => {
            is_multiline(ctx, node) && !method_name_and_arguments_on_same_line(ctx, node)
        }
        MultilineStyle::DiffComma => {
            is_multiline(ctx, node) && last_item_precedes_newline(ctx, node)
        }
        MultilineStyle::NoComma => false,
    }
}

/// RuboCop's `comma_offset`: the absolute position of a trailing comma in `range`, if `range`
/// starts with nothing but blanks (newlines excluded when a heredoc is among the items) then a
/// comma.
fn find_trailing_comma(ctx: &Context<'_>, range: Span, any_heredoc: bool) -> Option<u32> {
    let text = ctx.text(range);
    for (i, &b) in text.iter().enumerate() {
        if b == b',' {
            return Some(range.start + u32::try_from(i).unwrap_or(u32::MAX));
        }
        let blank = if any_heredoc { is_space_not_newline(b) } else { is_space(b) };
        if !blank {
            return None;
        }
    }
    None
}

/// RuboCop's `inside_comment?`: a same-line comment starting before `comma_pos` means the "comma"
/// found by [`find_trailing_comma`] is really inside that comment's text, not real Ruby syntax.
fn inside_comment(ctx: &Context<'_>, range: Span, comma_pos: u32) -> bool {
    let line = ctx.line_col(range.start).line;
    ctx.comments().iter().any(|c| c.line == line && c.span.start < comma_pos)
}

/// RuboCop's `extra_avoid_comma_info`.
fn extra_avoid_comma_info(style: MultilineStyle) -> &'static str {
    match style {
        MultilineStyle::Comma => ", unless each item is on its own line",
        MultilineStyle::ConsistentComma => ", unless items are split onto multiple lines",
        MultilineStyle::DiffComma => ", unless that item immediately precedes a newline",
        MultilineStyle::NoComma => "",
    }
}

/// Substitutes RuboCop's `%<article>s` placeholder in a `kind` template.
fn format_kind(kind: &str, article: &str) -> String {
    kind.replacen("%<article>s", article, 1)
}

/// RuboCop's `avoid_comma` message (`MSG` with `command: 'Avoid'`).
fn avoid_message(kind: &str, style: MultilineStyle) -> String {
    let article = if kind.contains("array") { "an" } else { "a" };
    format!(
        "Avoid comma after the last {}{}.",
        format_kind(kind, article),
        extra_avoid_comma_info(style)
    )
}

/// RuboCop's `put_comma` message (`MSG` with `command: 'Put a'`).
fn put_message(kind: &str) -> String {
    format!("Put a comma after the last {}.", format_kind(kind, "a multiline"))
}

/// RuboCop's `autocorrect_range`: the last item's own last "word" (skipping any leading lines of a
/// multiline item) through its end - the offense location and the position a missing comma is
/// inserted after.
fn autocorrect_anchor(ctx: &Context<'_>, item: Span) -> Span {
    let text = ctx.text(item);
    let after_newline = text.iter().rposition(|&b| b == b'\n').unwrap_or(0);
    let word_offset = text[after_newline..].iter().position(|&b| !is_space(b)).unwrap_or(0);
    let start = item.start + u32::try_from(after_newline + word_offset).unwrap_or(0);
    Span::new(start, item.end)
}

/// RuboCop's `TrailingComma#check`: reports (and, for a safe fix, corrects) a missing or forbidden
/// trailing comma for `node`, using `kind` (RuboCop's `'%<article>s item of...'`-shaped template,
/// e.g. `"parameter of %<article>s method call"`) to build the message.
pub fn check(
    ctx: &mut Context<'_>,
    meta: &'static RuleMeta,
    style: MultilineStyle,
    kind: &str,
    node: &TrailingCommaNode,
) {
    let after = Span::new(node.last_item.end, node.closing.start);
    let comma_pos = find_trailing_comma(ctx, after, node.any_heredoc)
        .filter(|&pos| !inside_comment(ctx, after, pos));
    let should = should_have_comma(ctx, style, node);

    match comma_pos {
        Some(pos) if !should => {
            let span = Span::new(pos, pos + 1);
            let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] };
            ctx.report_with_fix(meta, span, avoid_message(kind, style), fix);
        }
        None if should && !node.last_item_is_block_pass => {
            let anchor = autocorrect_anchor(ctx, node.last_item);
            let fix = Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::insert(anchor.end, b",".as_slice())],
            };
            ctx.report_with_fix(meta, anchor, put_message(kind), fix);
        }
        _ => {}
    }
}

/// RuboCop's `heredoc?`: true for a heredoc string/xstring itself, or (recursively) for a call
/// whose receiver (when it takes no arguments, e.g. `<<~SQL.strip`) or last argument (when it does,
/// e.g. `method(<<~SQL.strip)`) is one, or for a hash/keyword-hash/pair whose last element/value is
/// one.
#[must_use]
pub fn is_heredoc(ctx: &Context<'_>, node: &Node<'_>) -> bool {
    match node {
        Node::StringNode { .. } => {
            let n = node.as_string_node().expect("kind matched");
            n.opening_loc().is_some_and(|o| ctx.text(o.span()).starts_with(b"<<"))
        }
        Node::InterpolatedStringNode { .. } => {
            let n = node.as_interpolated_string_node().expect("kind matched");
            n.opening_loc().is_some_and(|o| ctx.text(o.span()).starts_with(b"<<"))
        }
        Node::XStringNode { .. } => {
            let n = node.as_x_string_node().expect("kind matched");
            ctx.text(n.opening_loc().span()).starts_with(b"<<")
        }
        Node::InterpolatedXStringNode { .. } => {
            let n = node.as_interpolated_x_string_node().expect("kind matched");
            ctx.text(n.opening_loc().span()).starts_with(b"<<")
        }
        Node::CallNode { .. } => {
            let call = node.as_call_node().expect("kind matched");
            match call.arguments() {
                None => call.receiver().is_some_and(|r| is_heredoc(ctx, &r)),
                Some(args) => args.arguments().last().is_some_and(|a| is_heredoc(ctx, &a)),
            }
        }
        Node::AssocNode { .. } => {
            let assoc = node.as_assoc_node().expect("kind matched");
            is_heredoc(ctx, &assoc.value())
        }
        Node::HashNode { .. } => {
            let hash = node.as_hash_node().expect("kind matched");
            hash.elements().last().is_some_and(|e| is_heredoc(ctx, &e))
        }
        Node::KeywordHashNode { .. } => {
            let hash = node.as_keyword_hash_node().expect("kind matched");
            hash.elements().last().is_some_and(|e| is_heredoc(ctx, &e))
        }
        _ => false,
    }
}

/// RuboCop's `any_heredoc?`.
#[must_use]
pub fn any_heredoc<'pr>(ctx: &Context<'_>, items: impl IntoIterator<Item = Node<'pr>>) -> bool {
    items.into_iter().any(|n| is_heredoc(ctx, &n))
}
