//! `Style/TrailingCommaInHashLiteral`, ported from RuboCop's
//! `lib/rubocop/cop/style/trailing_comma_in_hash_literal.rb`, which drives
//! the shared `TrailingComma` mixin
//! (`lib/rubocop/cop/mixin/trailing_comma.rb`) with `on_hash`.
//!
//! The mixin is also used by `Style/TrailingCommaInArguments` and
//! `Style/TrailingCommaInArrayLiteral`; this file ports the logic locally
//! rather than sharing code, since a `HashNode`'s children are always
//! `pair`/`kwsplat` nodes (never flattened call arguments), which simplifies
//! several of the mixin's helpers away.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, HashNode, NodeList};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `TrailingComma::MSG` interpolated for `command: 'Put a'`: the
/// message never varies by style once a comma is required.
const PUT_MSG: &str = "Put a comma after the last item of a multiline hash.";

/// The four `EnforcedStyleForMultiline` values, in `config/default.yml`
/// order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Comma,
    ConsistentComma,
    DiffComma,
    NoComma,
}

impl Style {
    /// RuboCop's `extra_avoid_comma_info`: the clause appended to the
    /// "Avoid comma" message explaining when a comma would be required.
    const fn avoid_extra_info(self) -> &'static str {
        match self {
            Self::Comma => ", unless each item is on its own line",
            Self::ConsistentComma => ", unless items are split onto multiple lines",
            Self::DiffComma => ", unless that item immediately precedes a newline",
            Self::NoComma => "",
        }
    }
}

/// Checks for trailing comma in hash literals.
#[derive(Debug, Clone)]
pub struct TrailingCommaInHashLiteral {
    style: Style,
}

impl TrailingCommaInHashLiteral {
    /// RuboCop's `check_literal` + `check`: looks at the text between the
    /// last element and the closing brace for an existing trailing comma,
    /// and either flags it as unwanted or, when absent, as required.
    fn check_literal(&self, hash: &HashNode<'_>, ctx: &mut Context<'_>) {
        let elements = hash.elements();
        if elements.is_empty() {
            // A braceless hash (the last argument of a method call) is a
            // `KeywordHashNode` in Prism, never reaches here; this only
            // guards the empty-literal case `{}`.
            return;
        }
        let opening = hash.opening_loc().span();
        let closing = hash.closing_loc().span();
        let last = elements.last().expect("checked non-empty");
        let last_span = last.span();
        let heredoc = elements.iter().any(|item| item_has_heredoc(ctx, &item));

        let after_last = ctx.text(Span::new(last_span.end, closing.start));
        match comma_offset(after_last, heredoc) {
            Some(offset) => {
                let comma_pos = last_span.end + offset;
                if !self.should_have_comma(ctx, opening, &elements, last_span, closing) {
                    self.avoid_comma(ctx, comma_pos);
                }
            }
            None => {
                if self.should_have_comma(ctx, opening, &elements, last_span, closing) {
                    Self::put_comma(ctx, last_span);
                }
            }
        }
    }

    /// RuboCop's `should_have_comma?`.
    fn should_have_comma(
        &self,
        ctx: &Context<'_>,
        opening: Span,
        elements: &NodeList<'_>,
        last_span: Span,
        closing: Span,
    ) -> bool {
        match self.style {
            Style::NoComma => false,
            Style::Comma => {
                is_multiline(ctx, opening, elements, closing)
                    && no_elements_on_same_line(ctx, elements, closing)
            }
            Style::ConsistentComma => is_multiline(ctx, opening, elements, closing),
            Style::DiffComma => {
                is_multiline(ctx, opening, elements, closing)
                    && last_item_precedes_newline(ctx, last_span, closing)
            }
        }
    }

    /// RuboCop's `avoid_comma`: the comma is present but not wanted.
    fn avoid_comma(&self, ctx: &mut Context<'_>, comma_pos: u32) {
        let span = Span::new(comma_pos, comma_pos + 1);
        let message =
            format!("Avoid comma after the last item of a hash{}.", self.style.avoid_extra_info());
        let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }

    /// RuboCop's `put_comma`: the comma is required but missing.
    fn put_comma(ctx: &mut Context<'_>, last_span: Span) {
        let range = autocorrect_range(ctx, last_span);
        let fix =
            Fix { applicability: Applicability::Safe, edits: vec![Edit::insert(range.end, *b",")] };
        ctx.report_with_fix(&Self::META, range, PUT_MSG, fix);
    }
}

impl Rule for TrailingCommaInHashLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Style/TrailingCommaInHashLiteral",
        department: Department::Style,
        summary: "Checks for trailing comma in hash literals.",
        explanation: "\
The configuration options are:

* `consistent_comma`: Requires a comma after the last item of all non-empty,
  multiline hash literals.
* `comma`: Requires a comma after the last item in a hash, but only when
  each item is on its own line.
* `diff_comma`: Requires a comma after the last item in a hash, but only
  when that item is followed by an immediate newline, even if there is an
  inline comment on the same line.
* `no_comma` (default): Does not require a comma after the last item in a
  hash.

```ruby
# EnforcedStyleForMultiline: no_comma (default)

# bad
a = { foo: 1, bar: 2, }

# good
a = {
  foo: 1,
  bar: 2
}
```

```ruby
# EnforcedStyleForMultiline: comma

# bad
a = {
  foo: 1, bar: 2,
  qux: 3
}

# good
a = {
  foo: 1, bar: 2,
  qux: 3,
}
```

```ruby
# EnforcedStyleForMultiline: consistent_comma

# good
a = {
  foo: 1, bar: 2,
  qux: 3,
}

# good
a = {
  foo: 1,
  bar: 2,
}
```

```ruby
# EnforcedStyleForMultiline: diff_comma

# bad
a = { foo: 1, bar: 2,
      baz: 3, qux: 4, }

# good
a = { foo: 1, bar: 2,
      baz: 3, qux: 4 }
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::HashNode],
        config: &[ConfigOption {
            name: "EnforcedStyleForMultiline",
            default: ConfigDefault::Str("no_comma"),
            allowed: &["comma", "consistent_comma", "diff_comma", "no_comma"],
            doc: "Whether, and when, a multiline hash literal needs a trailing comma.",
        }],
        blind_spots: "\
RuboCop's mixin also guards a same-line comma that is actually inside a
trailing comment (`inside_comment?`), comparing the comment's start against
the matched comma's position. That guard is unreachable in practice: the
comma-detection scan requires an unbroken run of whitespace immediately
before the comma, and any interposed `#` comment always breaks that run
first, so the guard's condition can never be reached from a case the scan
already accepts. It is therefore not reproduced here.

Heredoc detection (needed to keep the same-line comma scan from crossing
into a heredoc body) covers a pair's value being a heredoc string/xstring
directly, or reachable through a chain of method calls with no arguments
(recursing into the receiver) or with arguments (recursing into the last
one), matching the mixin's `heredoc?`/`heredoc_send?`. Anything deeper
(e.g. a heredoc nested inside a collection literal value) is not detected
and may cause a false positive comma match inside the heredoc body.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyleForMultiline")? {
            "comma" => Style::Comma,
            "consistent_comma" => Style::ConsistentComma,
            "diff_comma" => Style::DiffComma,
            "no_comma" => Style::NoComma,
            _ => unreachable!("RuleOptions::style validates against `allowed`"),
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Node::HashNode { .. } = node else { return };
        let hash = node.as_hash_node().expect("kind matched");
        self.check_literal(&hash, ctx);
    }
}

/// RuboCop's `Node#multiline?` (`first_line != last_line` of the node's own
/// source range, i.e. from the opening to the closing brace) combined with
/// `!allowed_multiline_argument?`: a single element whose closing brace
/// does not begin its own line is exempt.
fn is_multiline(ctx: &Context<'_>, opening: Span, elements: &NodeList<'_>, closing: Span) -> bool {
    let first_line = ctx.line_col(opening.start).line;
    let last_line = ctx.line_col(closing.end.saturating_sub(1)).line;
    if first_line == last_line {
        return false;
    }
    let allowed_single = elements.len() == 1 && !begins_its_line(ctx, closing.start);
    !allowed_single
}

/// RuboCop's `no_elements_on_same_line?`: no two consecutive items (each
/// hash element, then the closing brace) share a line.
fn no_elements_on_same_line(ctx: &Context<'_>, elements: &NodeList<'_>, closing: Span) -> bool {
    let mut boundaries: Vec<(u32, u32)> = elements
        .iter()
        .map(|item| {
            let span = item.span();
            (ctx.line_col(span.start).line, ctx.line_col(span.end.saturating_sub(1)).line)
        })
        .collect();
    let closing_line = ctx.line_col(closing.start).line;
    boundaries.push((closing_line, closing_line));
    boundaries.windows(2).all(|w| w[0].1 != w[1].0)
}

/// RuboCop's `last_item_precedes_newline?`: the text from the end of the
/// last element to the end of the whole hash node (comma, comment, and
/// closing brace included) starts, after an optional comma, a run of
/// whitespace, and an optional end-of-line comment, with a newline.
fn last_item_precedes_newline(ctx: &Context<'_>, last_span: Span, closing: Span) -> bool {
    let text = ctx.text(Span::new(last_span.end, closing.end));
    let start = usize::from(text.first() == Some(&b','));
    let mut ws_end = start;
    while ws_end < text.len() && is_ruby_space(text[ws_end]) {
        ws_end += 1;
    }
    if text[start..ws_end].contains(&b'\n') {
        return true;
    }
    text.get(ws_end) == Some(&b'#') && text[ws_end..].contains(&b'\n')
}

/// RuboCop's `Util.begins_its_line?`: whether `pos` is the first
/// non-whitespace character on its line.
fn begins_its_line(ctx: &Context<'_>, pos: u32) -> bool {
    let line = ctx.line_col(pos).line;
    let line_start = ctx.line_span(line).start;
    let text = ctx.line_text(line);
    let Some(first_non_ws) = text.iter().position(|&b| !is_ruby_space(b)) else {
        return false;
    };
    let first_non_ws_pos = line_start + u32::try_from(first_non_ws).unwrap_or(0);
    ctx.line_col(first_non_ws_pos).column == ctx.line_col(pos).column
}

/// Ruby's `[ \t\r\n\f\v]` (`\s`).
const fn is_ruby_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0x0C | 0x0B)
}

/// Ruby's `[^\S\n]` (`\s` minus `\n`): horizontal whitespace only.
const fn is_ruby_blank(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | 0x0C | 0x0B)
}

/// RuboCop's `comma_offset`: the index of the first `,` in `text`,
/// provided only whitespace precedes it (only horizontal whitespace when a
/// heredoc is among the items, so the scan cannot cross into its body).
fn comma_offset(text: &[u8], heredoc: bool) -> Option<u32> {
    let mut i = 0usize;
    while i < text.len() {
        match text[i] {
            b',' => return Some(u32::try_from(i).unwrap_or(u32::MAX)),
            b if (if heredoc { is_ruby_blank(b) } else { is_ruby_space(b) }) => i += 1,
            _ => break,
        }
    }
    None
}

/// RuboCop's `autocorrect_range`: the last element's own span, trimmed to
/// start at the first non-whitespace character after its last embedded
/// newline (a no-op for single-line elements).
fn autocorrect_range(ctx: &Context<'_>, item_span: Span) -> Span {
    let text = ctx.text(item_span);
    let from_last_newline = text.iter().rposition(|&b| b == b'\n').unwrap_or(0);
    let rest = &text[from_last_newline..];
    let non_ws = rest.iter().position(|&b| !is_ruby_space(b)).unwrap_or(0);
    let start = item_span.start + u32::try_from(from_last_newline + non_ws).unwrap_or(0);
    Span::new(start, item_span.end)
}

/// RuboCop's `heredoc?`/`heredoc_send?` restricted to a hash element: does
/// its value (recursing through receiver-only or last-argument call
/// chains) resolve to a heredoc string/xstring?
fn item_has_heredoc(ctx: &Context<'_>, item: &Node<'_>) -> bool {
    if let Some(assoc) = item.as_assoc_node() {
        return is_heredoc_value(ctx, &assoc.value());
    }
    if let Some(splat) = item.as_assoc_splat_node() {
        return splat.value().is_some_and(|value| is_heredoc_value(ctx, &value));
    }
    false
}

fn is_heredoc_value(ctx: &Context<'_>, node: &Node<'_>) -> bool {
    if let Some(opening) = heredoc_opening_span(node) {
        return ctx.text(opening).starts_with(b"<<");
    }
    if let Some(call) = node.as_call_node() {
        return is_heredoc_call(ctx, &call);
    }
    false
}

fn is_heredoc_call(ctx: &Context<'_>, call: &CallNode<'_>) -> bool {
    let last_arg = call.arguments().and_then(|args| args.arguments().last());
    match last_arg {
        Some(arg) => is_heredoc_value(ctx, &arg),
        None => call.receiver().is_some_and(|receiver| is_heredoc_value(ctx, &receiver)),
    }
}

/// The opening-delimiter span of a string-like node, if it is one.
fn heredoc_opening_span(node: &Node<'_>) -> Option<Span> {
    match node {
        Node::StringNode { .. } => {
            node.as_string_node().and_then(|n| n.opening_loc()).map(|l| l.span())
        }
        Node::InterpolatedStringNode { .. } => {
            node.as_interpolated_string_node().and_then(|n| n.opening_loc()).map(|l| l.span())
        }
        Node::XStringNode { .. } => {
            Some(node.as_x_string_node().expect("kind matched").opening_loc().span())
        }
        Node::InterpolatedXStringNode { .. } => {
            Some(node.as_interpolated_x_string_node().expect("kind matched").opening_loc().span())
        }
        _ => None,
    }
}
