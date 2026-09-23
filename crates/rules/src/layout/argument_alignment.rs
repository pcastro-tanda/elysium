//! `Layout/ArgumentAlignment`, ported from RuboCop's
//! `lib/rubocop/cop/layout/argument_alignment.rb` plus the `Alignment` mixin
//! (`lib/rubocop/cop/mixin/alignment.rb`) and `AlignmentCorrector`
//! (`lib/rubocop/cop/correctors/alignment_corrector.rb`) it uses for
//! autocorrection.
//!
//! Every call this rule needs (the argument list, the last/first argument,
//! a braceless keyword hash's pairs) is a direct Prism accessor off the
//! `CallNode` itself, so unlike rules that need ancestor bookkeeping this
//! one does all its work in a single `enter` with no facts gathered on
//! other node kinds. Prism gives a braceless bare-keyword-argument list
//! (`foo a: 1`) its own [`NodeKind::KeywordHashNode`], distinct from a
//! braced hash literal's [`NodeKind::HashNode`] -- this maps directly onto
//! RuboCop-AST's `hash_type?`/`braces?` split used throughout the cop.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{walk, LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

/// RuboCop's `ALIGN_PARAMS_MSG`.
const ALIGN_PARAMS_MSG: &str =
    "Align the arguments of a method call if they span more than one line.";
/// RuboCop's `FIXED_INDENT_MSG`.
const FIXED_INDENT_MSG: &str = "Use one level of indentation for arguments following the \
                                 first line of a multi-line method call.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Align every following line with the call's first argument.
    WithFirstArgument,
    /// Indent every following line one configured width past the line the
    /// method call starts on, regardless of where the first argument sits.
    WithFixedIndentation,
}

/// Checks that the arguments of a multi-line method call are aligned,
/// ported from RuboCop's `ArgumentAlignment` cop plus its `Alignment`
/// mixin and `AlignmentCorrector`.
#[derive(Debug, Clone)]
pub struct ArgumentAlignment {
    style: Style,
    /// RuboCop's `configured_indentation_width`.
    indentation_width: i64,
    /// RuboCop's `autocorrect_incompatible_with_other_cops?`: `with_first_argument` style
    /// combined with `Layout/HashAlignment` configured for a "separator" alignment style is a
    /// cross-cop conflict neither cop can resolve alone, so this cop disables itself entirely.
    disabled_by_hash_alignment: bool,
    /// Every span already reported in this file (RuboCop's per-investigation
    /// `current_offenses`): an offense whose range falls inside one already reported is emitted
    /// without a fix (two rewrites of the same region in one pass cannot be handled; the next
    /// fix iteration finds it again).
    reported: Vec<Span>,
}

impl ArgumentAlignment {
    /// RuboCop's `message`.
    const fn message(&self) -> &'static str {
        match self.style {
            Style::WithFixedIndentation => FIXED_INDENT_MSG,
            Style::WithFirstArgument => ALIGN_PARAMS_MSG,
        }
    }

    /// RuboCop's `flattened_arguments`.
    fn flattened_arguments<'pr>(&self, args: Vec<Node<'pr>>) -> Vec<Node<'pr>> {
        match self.style {
            Style::WithFixedIndentation => arguments_with_last_arg_pairs(args),
            Style::WithFirstArgument => arguments_or_first_arg_pairs(args),
        }
    }

    /// RuboCop's `base_column`. `first` is `items.first` (not necessarily
    /// `call`'s own first argument: a braceless hash's pairs may have
    /// replaced it).
    fn base_column(&self, call: &CallNode<'_>, first: Option<&Node<'_>>, ctx: &Context<'_>) -> i64 {
        if let (Style::WithFirstArgument, Some(first)) = (self.style, first) {
            i64::from(display_column(ctx, first.span()))
        } else {
            let line = target_method_lineno(call, ctx);
            i64::from(indentation_of_line(ctx, line)) + self.indentation_width
        }
    }

    /// RuboCop's `Alignment#check_alignment` + `#each_bad_alignment`.
    fn check_alignment(&mut self, ctx: &mut Context<'_>, items: &[Node<'_>], base_column: i64) {
        let mut prev_line: i64 = -1;
        for item in items {
            let span = item.span();
            let line = i64::from(ctx.line_col(span.start).line);
            if line > prev_line && begins_its_line(ctx, span) {
                let column_delta = base_column - i64::from(display_column(ctx, span));
                if column_delta != 0 {
                    self.register_offense(ctx, item, column_delta);
                }
            }
            prev_line = line;
        }
    }

    /// RuboCop's `Alignment#register_offense`.
    fn register_offense(&mut self, ctx: &mut Context<'_>, item: &Node<'_>, column_delta: i64) {
        let span = item.span();
        let message = self.message();
        let nested = self.reported.iter().any(|reported| reported.contains(span));
        self.reported.push(span);
        if nested {
            ctx.report(&Self::META, span, message);
            return;
        }
        match build_fix(ctx, item, column_delta) {
            Some(fix) => ctx.report_with_fix(&Self::META, span, message, fix),
            None => ctx.report(&Self::META, span, message),
        }
    }
}

impl Rule for ArgumentAlignment {
    const META: RuleMeta = RuleMeta {
        name: "Layout/ArgumentAlignment",
        department: Department::Layout,
        summary: "Align the arguments of a method call if they span more than one line.",
        explanation: "\
Checks that the arguments on a multi-line method call are aligned.

```ruby
# EnforcedStyle: with_first_argument (default)

# good

foo :bar,
    :baz,
    key: value

foo(
  :bar,
  :baz,
  key: value
)

# bad

foo :bar,
  :baz,
  key: value

foo(
  :bar,
    :baz,
    key: value
)
```

```ruby
# EnforcedStyle: with_fixed_indentation

# good

foo :bar,
  :baz,
  key: value

# bad

foo :bar,
    :baz,
    key: value
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("with_first_argument"),
                allowed: &["with_first_argument", "with_fixed_indentation"],
                doc: "Aligns following lines with the first argument \
                      (`with_first_argument`) or one indentation level past \
                      the method call's own line (`with_fixed_indentation`).",
            },
            ConfigOption {
                name: "IndentationWidth",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "Overrides `Layout/IndentationWidth`'s configured width \
                      for `with_fixed_indentation`'s base column; falls back \
                      to it, else 2.",
            },
        ],
        blind_spots: "\
`display_column` approximates Ruby's `unicode-display_width` gem with a
hand-rolled East Asian Width table covering the common CJK, Hangul, and
fullwidth-forms ranges; combining marks, emoji sequences, and rarer wide
code points are not modeled and could misalign a comparison in exotic
source files.

Autocorrection's taboo-range protection (RuboCop's `AlignmentCorrector`
`inside_string_ranges`) only covers heredoc bodies; the interior of an
ordinary multi-line quoted string or `%`-literal that itself begins a
physical line inside a misaligned argument is not separately protected.
The block-comment guard is a per-line `=begin` text match rather than
resolving actual `EmbDoc` comment nodes, matching this crate's other
`Alignment`-based cops.

`autocorrect_incompatible_with_other_cops?` (a `with_first_argument`-style
conflict with `Layout/HashAlignment` configured for a `separator` alignment
style) is honoured through `RuleOptions::peer`, which only sees `Layout/
HashAlignment` options explicitly set in the loaded config, not that cop's
own defaults merged in; since the default `EnforcedHashRocketStyle`/
`EnforcedColonStyle` is `key`, not `separator`, this matches RuboCop's
actual behaviour for every config that does not explicitly opt in.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "with_fixed_indentation" => Style::WithFixedIndentation,
            _ => Style::WithFirstArgument,
        };
        let indentation_width = options
            .get("IndentationWidth")
            .and_then(OptionValue::as_int)
            .or_else(|| {
                options.peer("Layout/IndentationWidth", "Width").and_then(OptionValue::as_int)
            })
            .unwrap_or(2);
        let disabled_by_hash_alignment = style == Style::WithFirstArgument
            && ["EnforcedColonStyle", "EnforcedHashRocketStyle"]
                .into_iter()
                .any(|key| peer_uses_separator(options, key));
        Ok(Self { style, indentation_width, disabled_by_hash_alignment, reported: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.reported.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.disabled_by_hash_alignment {
            return;
        }
        let Node::CallNode { .. } = node else { return };
        let call = node.as_call_node().expect("kind matched");
        if call.name().as_slice() == b"[]=" && !call.is_safe_navigation() {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
        if !multiple_arguments(&args) {
            return;
        }
        let items = self.flattened_arguments(args);
        let base_column = self.base_column(&call, items.first(), ctx);
        self.check_alignment(ctx, &items, base_column);
    }
}

/// RuboCop's `HashAlignment::SEPARATOR_ALIGNMENT_STYLES` check: `config.for_enabled_cop
/// ('Layout/HashAlignment')[style]&.include?('separator')`. RuboCop allows either a lone style
/// string or a list of styles for these keys.
fn peer_uses_separator(options: &RuleOptions, key: &str) -> bool {
    match options.peer("Layout/HashAlignment", key) {
        Some(OptionValue::Str(s)) => s == "separator",
        Some(OptionValue::List(items)) => items.iter().any(|v| v.as_str() == Some("separator")),
        _ => false,
    }
}

/// RuboCop's `multiple_arguments?`.
fn multiple_arguments(args: &[Node<'_>]) -> bool {
    if args.len() >= 2 {
        return true;
    }
    args.first().and_then(hash_pairs).is_some_and(|pairs| pairs.len() >= 2)
}

/// RuboCop-AST's `HashNode#pairs`: every `AssocNode` element, excluding an `AssocSplatNode`
/// (`**`) double-splat entry. Works for both a braced `HashNode` and a braceless
/// `KeywordHashNode` alike (RuboCop-AST's `hash_type?` covers both; Prism gives each its own
/// `NodeKind`). `None` when `node` is not a hash at all.
fn hash_pairs<'pr>(node: &Node<'pr>) -> Option<Vec<Node<'pr>>> {
    let elements = if let Some(kw) = node.as_keyword_hash_node() {
        kw.elements()
    } else {
        node.as_hash_node()?.elements()
    };
    Some(elements.iter().filter(|el| el.as_assoc_splat_node().is_none()).collect())
}

/// RuboCop-AST's `braces?` being false for a hash node: Prism's `KeywordHashNode` is precisely
/// a braceless bare-keyword-argument hash (`foo a: 1`); a braced hash literal is always a
/// `HashNode`, even as a sole call argument.
fn is_braceless_hash(node: &Node<'_>) -> bool {
    node.as_keyword_hash_node().is_some()
}

/// RuboCop's `arguments_with_last_arg_pairs` (`EnforcedStyle: with_fixed_indentation`): every
/// argument but the last, plus the last argument itself, or its pairs when it is a braceless
/// hash.
fn arguments_with_last_arg_pairs(mut args: Vec<Node<'_>>) -> Vec<Node<'_>> {
    let Some(last) = args.pop() else { return args };
    if is_braceless_hash(&last) {
        if let Some(pairs) = hash_pairs(&last) {
            args.extend(pairs);
            return args;
        }
    }
    args.push(last);
    args
}

/// RuboCop's `arguments_or_first_arg_pairs` (`EnforcedStyle: with_first_argument`): the first
/// argument's pairs when it is a braceless hash, else every argument unmodified.
fn arguments_or_first_arg_pairs(args: Vec<Node<'_>>) -> Vec<Node<'_>> {
    if let Some(first) = args.first() {
        if is_braceless_hash(first) {
            if let Some(pairs) = hash_pairs(first) {
                return pairs;
            }
        }
    }
    args
}

/// RuboCop's `target_method_lineno`: the line of the call's method-name token, or -- for a
/// call with no textual name, `.()`  or an index `a[...]`/`a[...] = ...` -- the line its
/// opening delimiter starts on.
fn target_method_lineno(call: &CallNode<'_>, ctx: &Context<'_>) -> u32 {
    let span = call
        .message_loc()
        .map(|loc| loc.span())
        .or_else(|| call.opening_loc().map(|loc| loc.span()))
        .unwrap_or_else(|| call.location().span());
    ctx.line_col(span.start).line
}

/// RuboCop's `/\S.*/.match(line).begin(0)`: the byte column of the first non-whitespace
/// character on `line`, or `0` for a blank line.
fn indentation_of_line(ctx: &Context<'_>, line: u32) -> u32 {
    let text = ctx.line_text(line);
    text.iter()
        .position(|&b| !b.is_ascii_whitespace())
        .map_or(0, |pos| u32::try_from(pos).unwrap_or(u32::MAX))
}

/// RuboCop's `Alignment#display_column`: the rendered width, in Unicode East Asian Width
/// terms, of the text preceding `span` on its own line.
fn display_column(ctx: &Context<'_>, span: Span) -> u32 {
    let line_col = ctx.line_col(span.start);
    let line = ctx.line_text(line_col.line);
    let take = usize::try_from(line_col.column).unwrap_or(usize::MAX);
    match std::str::from_utf8(line) {
        Ok(text) => text.chars().take(take).map(east_asian_width).sum(),
        Err(_) => line_col.column,
    }
}

/// Approximates Ruby's `unicode-display_width` gem: 2 columns for East Asian Wide and
/// Fullwidth code points, 1 otherwise. Covers the common CJK, Hangul, and fullwidth-forms
/// ranges (see `META.blind_spots`).
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

/// RuboCop's `Util#begins_its_line?`, character-based (matching Ruby's `String#index`/
/// `Range#column`) so it also holds on lines with non-ASCII leading content.
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

/// RuboCop's `AlignmentCorrector.correct`: shifts every physical line of `item` by
/// `column_delta` columns. Returns `None` when nothing could be safely edited (a
/// `=begin`/`=end` block comment inside the range, or every line was blocked by a taboo
/// range/whitespace mismatch).
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

/// RuboCop's `AlignmentCorrector#inside_string_ranges`'s heredoc case: collects heredoc body
/// ranges within a subtree so autocorrection never touches heredoc content (see
/// `META.blind_spots` for the non-heredoc delimited-literal case this does not cover).
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
