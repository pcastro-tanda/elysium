//! `Style/TrailingCommaInArrayLiteral`, ported from RuboCop's
//! `lib/rubocop/cop/style/trailing_comma_in_array_literal.rb` (which mixes in
//! `lib/rubocop/cop/mixin/trailing_comma.rb`).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::Node;
use ruby_ast::{LocationExt as _, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `EnforcedStyleForMultiline`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    NoComma,
    Comma,
    ConsistentComma,
    DiffComma,
}

impl Style {
    /// RuboCop's `extra_avoid_comma_info`: appended to the "Avoid comma..."
    /// message, describing when the comma would in fact be wanted.
    const fn extra_avoid_comma_info(self) -> &'static str {
        match self {
            Self::NoComma => "",
            Self::Comma => ", unless each item is on its own line",
            Self::ConsistentComma => ", unless items are split onto multiple lines",
            Self::DiffComma => ", unless that item immediately precedes a newline",
        }
    }
}

/// `Style/TrailingCommaInArrayLiteral`.
#[derive(Debug, Clone)]
pub struct TrailingCommaInArrayLiteral {
    style: Style,
}

impl Rule for TrailingCommaInArrayLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Style/TrailingCommaInArrayLiteral",
        department: Department::Style,
        summary: "Checks for trailing comma in array literals.",
        explanation: "\
The configuration options are:

* `consistent_comma`: Requires a comma after the last item of all non-empty,
  multiline array literals.
* `comma`: Requires a comma after the last item in an array, but only when
  each item is on its own line.
* `diff_comma`: Requires a comma after the last item in an array, but only
  when that item is followed by an immediate newline, even if there is an
  inline comment on the same line.
* `no_comma` (default): Does not require a comma after the last item in an
  array.

```ruby
# EnforcedStyleForMultiline: no_comma (default)

# bad
a = [1, 2,]

# good
a = [
  1,
  2
]
```

```ruby
# EnforcedStyleForMultiline: comma

# bad
a = [
  1, 2,
  3,
]

# good
a = [
  1, 2,
  3
]
```

```ruby
# EnforcedStyleForMultiline: consistent_comma

# good
a = [
  1, 2,
  3,
]

# good
a = [
  1, 2, 3,
]
```

```ruby
# EnforcedStyleForMultiline: diff_comma

# bad
a = [1, 2,
     3, 4,]

# good
a = [1, 2,
     3, 4]
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ArrayNode],
        config: &[ConfigOption {
            name: "EnforcedStyleForMultiline",
            default: ConfigDefault::Str("no_comma"),
            allowed: &["comma", "consistent_comma", "diff_comma", "no_comma"],
            doc: "The comma style to require for multiline array literals.",
        }],
        blind_spots: "\
Heredoc detection only follows direct heredoc string literals and method
chains rooted at a heredoc receiver or ending in a heredoc argument (RuboCop's
`heredoc_send?`); a heredoc nested inside a hash-literal array element's value
(RuboCop's `pair`/`hash` case of `heredoc?`) is not recognised, which can only
turn a false positive into a rarer false negative (a trailing-comma-in-comma
misdetection next to such a value), consistent with the false-negatives-over-
false-positives policy.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyleForMultiline")? {
            "comma" => Style::Comma,
            "consistent_comma" => Style::ConsistentComma,
            "diff_comma" => Style::DiffComma,
            _ => Style::NoComma,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Node::ArrayNode { .. } = node else { return };
        let array = node.as_array_node().expect("kind matched");

        // RuboCop's `on_array` only checks `[...]` literals: percent arrays
        // (`%w[...]`, `%i(...)`) and array patterns are exempt.
        let Some(opening) = array.opening_loc() else { return };
        if opening.as_slice() != b"[" {
            return;
        }
        let Some(closing) = array.closing_loc() else { return };

        let elements = array.elements();
        if elements.is_empty() {
            return;
        }

        let element_spans: Vec<Span> = elements.iter().map(|element| element.span()).collect();
        let last_span = *element_spans.last().expect("checked non-empty");
        let closing_span = closing.span();
        let after_last_item = Span::new(last_span.end, closing_span.start);
        let text = ctx.text(after_last_item);

        let any_heredoc = elements.iter().any(|element| is_heredoc(&element));
        let comma_offset = find_comma_offset(text, any_heredoc);

        let multiline = is_multiline(ctx, node.span(), element_spans.len(), closing_span);
        let should_have_comma = multiline
            && match self.style {
                Style::NoComma => false,
                Style::Comma => no_elements_on_same_line(ctx, &element_spans, closing_span),
                Style::ConsistentComma => true,
                Style::DiffComma => last_item_precedes_newline(text),
            };

        let real_comma =
            comma_offset.filter(|&offset| !inside_comment(ctx, after_last_item, offset));

        match real_comma {
            Some(offset) => {
                if !should_have_comma {
                    let comma_pos = after_last_item.start + u32::try_from(offset).unwrap_or(0);
                    avoid_comma(ctx, &Self::META, self.style, comma_pos);
                }
            }
            None => {
                if should_have_comma {
                    put_comma(ctx, &Self::META, last_span);
                }
            }
        }
    }
}

/// RuboCop's `avoid_comma`: the comma is unwanted, report and remove it.
fn avoid_comma(ctx: &mut Context<'_>, meta: &'static RuleMeta, style: Style, comma_pos: u32) {
    let range = Span::new(comma_pos, comma_pos + 1);
    let message =
        format!("Avoid comma after the last item of an array{}.", style.extra_avoid_comma_info());
    ctx.report_with_fix(
        meta,
        range,
        message,
        Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(range)] },
    );
}

/// RuboCop's `put_comma`: the comma is required but missing, report and add
/// it right after the last item.
fn put_comma(ctx: &mut Context<'_>, meta: &'static RuleMeta, last_span: Span) {
    let range = autocorrect_range(ctx, last_span);
    let message = "Put a comma after the last item of a multiline array.";
    ctx.report_with_fix(
        meta,
        range,
        message,
        Fix { applicability: Applicability::Safe, edits: vec![Edit::insert(last_span.end, *b",")] },
    );
}

/// RuboCop's `autocorrect_range`: for a single-line item this is the whole
/// item; for an item whose own source spans multiple lines (e.g. a nested
/// multiline literal), only the last line's non-blank suffix is highlighted,
/// while the insertion point (the range's end) stays at the item's own end.
fn autocorrect_range(ctx: &Context<'_>, item_span: Span) -> Span {
    let text = ctx.text(item_span);
    let last_newline = text.iter().rposition(|&b| b == b'\n').unwrap_or(0);
    let rel = text[last_newline..].iter().position(|&b| !b.is_ascii_whitespace()).unwrap_or(0);
    let start = item_span.start + u32::try_from(last_newline + rel).unwrap_or(0);
    Span::new(start, item_span.end)
}

/// RuboCop's `comma_offset`: the byte offset of a real trailing comma within
/// `text` (the source between the last item and the closing bracket), or
/// `None` if the region isn't just (heredoc-restricted) whitespace followed
/// by a comma.
fn find_comma_offset(text: &[u8], any_heredoc: bool) -> Option<usize> {
    for (i, &byte) in text.iter().enumerate() {
        match byte {
            b',' => return Some(i),
            b'\n' if any_heredoc => return None,
            b' ' | b'\t' | b'\r' | 0x0b | 0x0c | b'\n' => {}
            _ => return None,
        }
    }
    None
}

/// RuboCop's `inside_comment?`: true when a same-line trailing comment
/// starts before the found comma, i.e. the "comma" isn't really code.
fn inside_comment(ctx: &Context<'_>, range: Span, offset: usize) -> bool {
    let line = ctx.line_col(range.start).line;
    let comma_pos = range.start + u32::try_from(offset).unwrap_or(0);
    ctx.comments().iter().any(|comment| comment.line == line && comment.span.start < comma_pos)
}

/// RuboCop's `last_item_precedes_newline?`: the text right after the last
/// item is (an optional existing comma, then) only whitespace and at most
/// one trailing comment before the next newline.
fn last_item_precedes_newline(text: &[u8]) -> bool {
    let mut i = usize::from(text.first() == Some(&b','));
    let mut saw_newline = false;
    while i < text.len() {
        match text[i] {
            b'\n' => {
                saw_newline = true;
                i += 1;
            }
            b' ' | b'\t' | b'\r' | 0x0b | 0x0c => i += 1,
            b'#' => {
                i += 1;
                while i < text.len() && text[i] != b'\n' {
                    i += 1;
                }
                return i < text.len();
            }
            _ => return false,
        }
    }
    saw_newline
}

/// RuboCop's `no_elements_on_same_line?`: every element, and the closing
/// bracket after it, starts on a line after the previous one's last line.
fn no_elements_on_same_line(ctx: &Context<'_>, elements: &[Span], closing: Span) -> bool {
    let mut prev_last_line: Option<u32> = None;
    for &span in elements.iter().chain(std::iter::once(&closing)) {
        let first_line = ctx.line_col(span.start).line;
        if prev_last_line == Some(first_line) {
            return false;
        }
        prev_last_line = Some(last_line_of(ctx, span));
    }
    true
}

/// RuboCop's `Node#multiline? && !allowed_multiline_argument?`: a single
/// element whose closing bracket doesn't begin its own line is exempt, since
/// then the array isn't considered "spread over multiple lines" even though
/// its source technically is.
fn is_multiline(ctx: &Context<'_>, node_span: Span, element_count: usize, closing: Span) -> bool {
    let first_line = ctx.line_col(node_span.start).line;
    let last_line = last_line_of(ctx, node_span);
    if first_line == last_line {
        return false;
    }
    let allowed_single_element = element_count == 1 && !begins_its_line(ctx, closing);
    !allowed_single_element
}

/// RuboCop's `Util.begins_its_line?`: `range` is the first non-blank thing on
/// its (1-based) source line.
fn begins_its_line(ctx: &Context<'_>, span: Span) -> bool {
    let line_col = ctx.line_col(span.start);
    let text = ctx.line_text(line_col.line);
    let column = line_col.column as usize;
    match std::str::from_utf8(text) {
        Ok(line) => line.chars().position(|c| !c.is_whitespace()) == Some(column),
        Err(_) => text.iter().position(|&b| !b.is_ascii_whitespace()) == Some(column),
    }
}

/// The 1-based line of the last byte covered by `span` (RuboCop's
/// `Range#last_line`).
fn last_line_of(ctx: &Context<'_>, span: Span) -> u32 {
    let last_byte = if span.start == span.end { span.start } else { span.end - 1 };
    ctx.line_col(last_byte).line
}

/// RuboCop's `heredoc?`: whether `node` is (or, for a method chain, is
/// rooted in) a heredoc string literal.
fn is_heredoc(node: &Node<'_>) -> bool {
    match node {
        Node::StringNode { .. } => node
            .as_string_node()
            .and_then(|n| n.opening_loc())
            .is_some_and(|loc| loc.as_slice().starts_with(b"<<")),
        Node::InterpolatedStringNode { .. } => node
            .as_interpolated_string_node()
            .and_then(|n| n.opening_loc())
            .is_some_and(|loc| loc.as_slice().starts_with(b"<<")),
        Node::XStringNode { .. } => {
            node.as_x_string_node().is_some_and(|n| n.opening_loc().as_slice().starts_with(b"<<"))
        }
        Node::InterpolatedXStringNode { .. } => node
            .as_interpolated_x_string_node()
            .is_some_and(|n| n.opening_loc().as_slice().starts_with(b"<<")),
        Node::CallNode { .. } => {
            let Some(call) = node.as_call_node() else { return false };
            match call.arguments() {
                Some(args) => args.arguments().last().is_some_and(|last| is_heredoc(&last)),
                None => call.receiver().is_some_and(|receiver| is_heredoc(&receiver)),
            }
        }
        _ => false,
    }
}
