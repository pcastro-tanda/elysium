//! `Layout/IndentationConsistency`, ported from RuboCop's
//! `lib/rubocop/cop/layout/indentation_consistency.rb` plus its `Alignment`
//! mixin and `AlignmentCorrector`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{walk, LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Inconsistent indentation detected.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Entities at the same logical depth share the same indentation,
    /// including `public`/`protected`/`private` markers themselves.
    Normal,
    /// Like `Normal`, but the members below a `protected`/`private` marker
    /// are indented one step further than the marker.
    IndentedInternalMethods,
}

/// Checks for inconsistent indentation, ported from RuboCop's
/// `IndentationConsistency` cop plus its `Alignment` mixin and
/// `AlignmentCorrector`.
#[derive(Debug, Clone)]
pub struct IndentationConsistency {
    style: Style,
    /// Spans already reported by this rule in this file, in visitation
    /// order (RuboCop's `@current_offenses`): an offense nested inside one
    /// already reported is emitted without a fix instead of colliding with
    /// the outer correction.
    reported: Vec<Span>,
}

impl Rule for IndentationConsistency {
    const META: RuleMeta = RuleMeta {
        name: "Layout/IndentationConsistency",
        department: Department::Layout,
        summary: "Keep indentation straight.",
        explanation: "\
Checks that entities at the same logical depth share the same indentation.
The `indented_internal_methods` style additionally requires that a bare
`protected`/`private`/`public`/`module_function` marker stay flush with the
surrounding methods, while the members below it are indented one step
further than the marker.

```ruby
# EnforcedStyle: normal (default)

# bad
class A
  def test
    puts 'hello'
     puts 'world'
  end
end

# good
class A
  def test
    puts 'hello'
    puts 'world'
  end

  protected

  def foo
  end
end
```

```ruby
# EnforcedStyle: indented_internal_methods

# bad
class A
  def test
    puts 'hello'
     puts 'world'
  end
end

# good
class A
  def test
    puts 'hello'
    puts 'world'
  end

  protected

    def foo
    end
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::StatementsNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("normal"),
            allowed: &["normal", "indented_internal_methods"],
            doc: "Whether `protected`/`private` markers must be indented \
                  like the surrounding methods (`normal`) or may sit flush \
                  with a deeper-indented internal section \
                  (`indented_internal_methods`).",
        }],
        blind_spots: "\
`bare_access_modifier?` does not replicate RuboCop's recursive
`in_macro_scope?` check (it does not confirm the call sits directly in a
class/module/top-level body through only `begin`/`kwbegin`/block/`if`
wrappers): any receiver-less, argument-less `public`/`protected`/`private`/
`module_function` call is treated as a group divider or excluded from
alignment, even one nested inside a `def` or loop body. That only narrows a
comparison into smaller groups (or drops one item entirely), never merges
two real groups into one, so it can only produce false negatives.
`private()`/`protected()` with empty parentheses are not recognized as bare
modifiers (RuboCop's AST does not distinguish them from a no-args call, but
our simpler receiver/arguments check does), which is a false-negative-only
divergence in the same direction.

`display_column` approximates Ruby's `unicode-display_width` gem with a
hand-rolled East Asian Width table covering the common CJK, Hangul, and
fullwidth-forms ranges; combining marks, emoji sequences, and rarer wide
code points are not modeled and could misalign a comparison in exotic
source files.

Autocorrection's taboo-range protection (RuboCop's `AlignmentCorrector`
`inside_string_ranges`) only covers heredoc bodies; the interior of an
ordinary multi-line quoted string or `%`-literal that itself begins a
physical line inside a misaligned body is not separately protected. The
block-comment guard is a per-line `=begin` text match rather than resolving
actual `EmbDoc` comment nodes, matching this crate's other `Alignment`-based
cops.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "indented_internal_methods" => Style::IndentedInternalMethods,
            _ => Style::Normal,
        };
        Ok(Self { style, reported: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.reported.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(statements) = node.as_statements_node() else { return };
        let body = statements.body();
        match self.style {
            Style::Normal => {
                let first_raw = body.first();
                let base_column = Self::base_column_for_normal_style(ctx, first_raw.as_ref());
                let items: Vec<Node<'_>> =
                    body.iter().filter(|child| !is_bare_access_modifier(child)).collect();
                self.check_alignment(ctx, &items, base_column);
            }
            Style::IndentedInternalMethods => {
                let mut groups: Vec<Vec<Node<'_>>> = vec![Vec::new()];
                for child in &body {
                    if is_bare_access_modifier(&child) {
                        groups.push(Vec::new());
                    } else {
                        groups.last_mut().expect("seeded with one group").push(child);
                    }
                }
                for group in groups {
                    self.check_alignment(ctx, &group, None);
                }
            }
        }
    }
}

impl IndentationConsistency {
    /// RuboCop's `base_column_for_normal_style`: `None` means "derive the
    /// base column from the first item being checked"; `Some` overrides it
    /// with the column of a bare access modifier that leads the raw
    /// (unfiltered) child list.
    fn base_column_for_normal_style(
        ctx: &Context<'_>,
        first_raw: Option<&Node<'_>>,
    ) -> Option<u32> {
        let first = first_raw?;
        if !is_bare_access_modifier(first) {
            return None;
        }
        let access_modifier_indent = display_column(ctx, first.span());
        let parent = ctx.parent()?;
        if parent.kind == NodeKind::ProgramNode {
            return Some(access_modifier_indent);
        }
        let parent_column = display_column(ctx, parent.span);
        (access_modifier_indent > parent_column).then_some(access_modifier_indent)
    }

    /// RuboCop's `Alignment#check_alignment` + `#each_bad_alignment`.
    fn check_alignment(
        &mut self,
        ctx: &mut Context<'_>,
        items: &[Node<'_>],
        base_column: Option<u32>,
    ) {
        let Some(first) = items.first() else { return };
        let base_column = base_column.unwrap_or_else(|| display_column(ctx, first.span()));
        let mut prev_line: i64 = -1;
        for item in items {
            let span = item.span();
            let line = i64::from(ctx.line_col(span.start).line);
            if line > prev_line && begins_its_line(ctx, span) {
                let column_delta = i64::from(base_column) - i64::from(display_column(ctx, span));
                if column_delta != 0 {
                    self.register_offense(ctx, item, column_delta);
                }
            }
            prev_line = line;
        }
    }

    /// RuboCop's `Alignment#register_offense`: an offense whose range falls
    /// inside one this rule already reported in this file is emitted
    /// without a fix (two rewrites of the same region in one pass cannot be
    /// handled; the next fix iteration finds it again).
    fn register_offense(&mut self, ctx: &mut Context<'_>, item: &Node<'_>, column_delta: i64) {
        let span = item.span();
        let nested = self.reported.iter().any(|reported| reported.contains(span));
        self.reported.push(span);
        if nested {
            ctx.report(&Self::META, span, MSG);
            return;
        }
        match build_fix(ctx, item, column_delta) {
            Some(fix) => ctx.report_with_fix(&Self::META, span, MSG, fix),
            None => ctx.report(&Self::META, span, MSG),
        }
    }
}

/// RuboCop's `bare_access_modifier?`: a receiver-less, argument-less call to
/// `public`/`protected`/`private`/`module_function` (see `META.blind_spots`
/// for the scope check this simplifies away).
fn is_bare_access_modifier(node: &Node<'_>) -> bool {
    let Node::CallNode { .. } = node else { return false };
    let call = node.as_call_node().expect("kind matched");
    if call.receiver().is_some() || call.arguments().is_some() {
        return false;
    }
    matches!(call.name().as_slice(), b"public" | b"protected" | b"private" | b"module_function")
}

/// RuboCop's `Alignment#display_column`: the rendered width, in Unicode
/// East Asian Width terms, of the text preceding `span` on its own line.
fn display_column(ctx: &Context<'_>, span: Span) -> u32 {
    let line_col = ctx.line_col(span.start);
    let line = ctx.line_text(line_col.line);
    let take = usize::try_from(line_col.column).unwrap_or(usize::MAX);
    match std::str::from_utf8(line) {
        Ok(text) => text.chars().take(take).map(east_asian_width).sum(),
        Err(_) => line_col.column,
    }
}

/// Approximates Ruby's `unicode-display_width` gem: 2 columns for East
/// Asian Wide and Fullwidth code points, 1 otherwise. Covers the common
/// CJK, Hangul, and fullwidth-forms ranges (see `META.blind_spots`).
fn east_asian_width(ch: char) -> u32 {
    let cp = u32::from(ch);
    let is_wide = matches!(cp,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x2_0000..=0x3_FFFD
    );
    if is_wide {
        2
    } else {
        1
    }
}

/// RuboCop's `Util#begins_its_line?`, character-based (matching Ruby's
/// `String#index`/`Range#column`) so it also holds on lines with
/// non-ASCII leading content.
fn begins_its_line(ctx: &Context<'_>, span: Span) -> bool {
    let line_col = ctx.line_col(span.start);
    let line = ctx.line_text(line_col.line);
    let Ok(text) = std::str::from_utf8(line) else { return line_col.column == 0 };
    match text.chars().position(|ch| !is_ruby_whitespace(ch)) {
        Some(index) => u32::try_from(index).unwrap_or(u32::MAX) == line_col.column,
        None => false,
    }
}

/// Ruby's `\s` character class, used by `begins_its_line?`'s regex.
fn is_ruby_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\r' | '\x0B' | '\x0C')
}

/// RuboCop's `AlignmentCorrector.correct`: shifts every physical line of
/// `item` by `column_delta` columns. Returns `None` when nothing could be
/// safely edited (a `=begin`/`=end` block comment inside the range, or
/// every line was blocked by a taboo range/whitespace mismatch).
fn build_fix(ctx: &Context<'_>, item: &Node<'_>, column_delta: i64) -> Option<Fix> {
    let span = item.span();
    let start_line = ctx.line_col(span.start).line;
    let last_byte = span.end.saturating_sub(1).max(span.start);
    let end_line = ctx.line_col(last_byte).line;

    for line in start_line..=end_line {
        if trim_start(ctx.line_text(line)).starts_with(b"=begin") {
            return None;
        }
    }

    let mut taboo = HeredocTaboo { ranges: Vec::new() };
    walk(item, &mut taboo);

    let mut edits = Vec::new();
    for line in start_line..=end_line {
        let is_first = line == start_line;
        let anchor = if is_first { span.start } else { ctx.line_span(line).start };

        if column_delta > 0 {
            let amount = u32::try_from(column_delta).unwrap_or(0);
            if !is_first && ctx.line_span(line).is_empty() {
                continue;
            }
            if taboo.ranges.iter().any(|t| t.contains(Span::empty(anchor))) {
                continue;
            }
            edits.push(Edit::insert(anchor, " ".repeat(amount as usize).into_bytes()));
        } else {
            let amount = u32::try_from(-column_delta).unwrap_or(0);
            let starts_with_space =
                ctx.source().bytes().get(anchor as usize).is_some_and(|&b| b == b' ');
            let range = if is_first || !starts_with_space {
                Span::new(anchor.saturating_sub(amount), anchor)
            } else {
                Span::new(anchor, anchor + amount)
            };
            if taboo.ranges.iter().any(|t| t.contains(range)) {
                continue;
            }
            let text = ctx.text(range);
            if !text.is_empty() && text.iter().all(|&b| b == b' ' || b == b'\t') {
                edits.push(Edit::delete(range));
            }
        }
    }
    if edits.is_empty() {
        None
    } else {
        Some(Fix { applicability: Applicability::Safe, edits })
    }
}

fn trim_start(text: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < text.len() && text[i].is_ascii_whitespace() {
        i += 1;
    }
    &text[i..]
}

/// RuboCop's `AlignmentCorrector#inside_string_ranges`'s heredoc case:
/// collects heredoc body ranges within a subtree so autocorrection never
/// touches heredoc content (see `META.blind_spots` for the non-heredoc
/// delimited-literal case this does not cover).
struct HeredocTaboo {
    ranges: Vec<Span>,
}

impl<'pr> Visitor<'pr> for HeredocTaboo {
    fn enter(&mut self, node: &Node<'pr>) {
        let opening_closing = match node {
            Node::StringNode { .. } => {
                let n = node.as_string_node().expect("kind matched");
                n.opening_loc().zip(n.closing_loc())
            }
            Node::InterpolatedStringNode { .. } => {
                let n = node.as_interpolated_string_node().expect("kind matched");
                n.opening_loc().zip(n.closing_loc())
            }
            Node::XStringNode { .. } => {
                let n = node.as_x_string_node().expect("kind matched");
                Some((n.opening_loc(), n.closing_loc()))
            }
            Node::InterpolatedXStringNode { .. } => {
                let n = node.as_interpolated_x_string_node().expect("kind matched");
                Some((n.opening_loc(), n.closing_loc()))
            }
            _ => None,
        };
        if let Some((open, close)) = opening_closing {
            if open.as_slice().starts_with(b"<<") {
                self.ranges.push(Span::new(open.span().end, close.span().start));
            }
        }
    }
}
