//! `Layout/FirstArgumentIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/first_argument_indentation.rb` plus the `Alignment` mixin
//! (`lib/rubocop/cop/mixin/alignment.rb`) and `AlignmentCorrector`
//! (`lib/rubocop/cop/correctors/alignment_corrector.rb`) it uses for autocorrection.
//!
//! Prism wraps a call's arguments in a real [`NodeKind::ArgumentsNode`] tree node, so an
//! argument's immediate parent in this traversal is that wrapper, not the enclosing call
//! (whitequark, which RuboCop is built on, has no such wrapper: arguments are direct children
//! of the `send` node). [`logical_parent`] bridges the gap by looking one level further up
//! whenever the immediate parent is an `ArgumentsNode`, so `eligible_method_call?` sees exactly
//! the node RuboCop's `node.parent` would.
//!
//! Facts about each open `CallNode`/`SuperNode` ancestor (needed for `eligible_method_call?`,
//! which Prism's plain kind+span ancestor stack cannot answer on its own) are recorded on
//! `enter` and popped on `leave`, following the single-traversal pattern used elsewhere in this
//! crate (see `style/hash_syntax.rs`).

use std::borrow::Cow;
use std::collections::HashSet;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    NodeInfo, OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{walk, LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// The first argument is always indented one step past the previous line.
    Consistent,
    /// The first argument is always indented one step past the receiver's own line.
    ConsistentRelativeToReceiver,
    /// Like the default, but without the parenthesized-outer-call restriction.
    SpecialForInnerMethodCall,
    /// The first argument of a call that is itself an argument of a parenthesized outer call is
    /// indented relative to that inner call; otherwise, one step past the previous line.
    SpecialForInnerMethodCallInParentheses,
}

/// Facts about one open `CallNode`/`SuperNode` ancestor, recorded on `enter` so a descendant
/// can answer RuboCop's `eligible_method_call?`/`parenthesized?` without re-walking up the tree
/// (which the plain kind+span ancestor stack cannot answer on its own).
#[derive(Debug, Clone, Copy, Default)]
struct CallFacts {
    /// `&.` was used (RuboCop's pattern `(send ...)` never matches a `csend` node).
    is_safe_navigation: bool,
    /// The method name is exactly `[]=` (RuboCop's `eligible_method_call?` pattern excludes it
    /// specifically, unlike plain `[]` or any other operator name).
    is_index_setter: bool,
    /// The call's arguments are wrapped in real parentheses (not `[...]` brackets, and not a
    /// bare/parenthesis-free command call).
    parenthesized: bool,
}

/// Checks the indentation of the first argument in a method call, ported from RuboCop's
/// `FirstArgumentIndentation` cop plus its `Alignment` mixin and `AlignmentCorrector`.
#[derive(Debug, Clone)]
pub struct FirstArgumentIndentation {
    style: Style,
    /// RuboCop's `configured_indentation_width`.
    width: i64,
    /// RuboCop's `enforce_first_argument_with_fixed_indentation? && \
    /// !enable_layout_first_method_argument_line_break?`, precomputed: neither operand can
    /// change per node, only per configuration.
    skip_with_fixed_indentation: bool,
    /// Facts about every `CallNode`/`SuperNode` ancestor currently open, outermost first.
    facts: Vec<CallFacts>,
    /// Every first-argument span already reported in this file (RuboCop's per-investigation
    /// `@current_offenses`): an offense whose span falls inside one already reported is emitted
    /// without a fix and with the generic message (two rewrites of the same region in one pass
    /// cannot be handled; the next fix iteration finds it again).
    reported: Vec<Span>,
    /// RuboCop's memoized `@comment_lines`: lines that are nothing but a comment, used by
    /// `previous_code_line` to skip past them. Built lazily, once per file.
    comment_lines: Option<HashSet<u32>>,
}

impl FirstArgumentIndentation {
    /// RuboCop's `should_check?` plus argument-presence, for a `send`/`csend` node: the first
    /// argument to check, if this call should be checked at all.
    fn call_target<'pr>(call: &CallNode<'pr>) -> Option<(Span, Node<'pr>)> {
        if call.equal_loc().is_some() {
            return None; // setter_method?
        }
        if is_operator_method(call.name().as_slice()) && call.call_operator_loc().is_none() {
            return None; // bare_operator?
        }
        let first = call.arguments()?.arguments().first()?;
        Some((call.location().span(), first))
    }

    /// RuboCop's `should_check?` plus argument-presence, for a `super` node (`bare_operator?`/
    /// `setter_method?` never hold for `super`).
    fn super_target<'pr>(sup: &ruby_ast::node::SuperNode<'pr>) -> Option<(Span, Node<'pr>)> {
        let first = sup.arguments()?.arguments().first()?;
        Some((sup.location().span(), first))
    }

    /// RuboCop's `on_send`/`on_csend`/`on_super` plus `Alignment#check_alignment` (called with a
    /// single-element `items` list, so `each_bad_alignment`'s `prev_line` bookkeeping is moot).
    fn check(&mut self, ctx: &mut Context<'_>, call_span: Span, first_arg: &Node<'_>) {
        let arg_span = first_arg.span();
        if same_line(ctx, call_span, arg_span) {
            return;
        }
        if self.skip_with_fixed_indentation {
            return;
        }
        if !begins_its_line(ctx, arg_span) {
            return;
        }

        let start = Self::base_range_start(ctx, call_span);
        let special = self.special_inner_call_indentation(ctx, call_span);
        let base_col = if special {
            self.column_of(ctx, start, arg_span.start)
        } else {
            i64::from(self.previous_code_line_indent(ctx, ctx.line_col(arg_span.start).line))
        };
        let indent = base_col + self.width;
        let actual = i64::from(ctx.display_column(arg_span.start));
        let delta = indent - actual;
        if delta == 0 {
            return;
        }

        let nested = self.reported.iter().any(|reported| reported.contains(arg_span));
        self.reported.push(arg_span);

        let message: Cow<'static, str> = if nested {
            Cow::Borrowed("Bad indentation of the first argument.")
        } else {
            Cow::Owned(Self::build_message(ctx, start, arg_span.start, special))
        };

        if nested {
            ctx.report(&Self::META, arg_span, message);
        } else {
            match build_shift_fix(ctx, first_arg, delta) {
                Some(fix) => ctx.report_with_fix(&Self::META, arg_span, message, fix),
                None => ctx.report(&Self::META, arg_span, message),
            }
        }
    }

    /// RuboCop's `special_inner_call_indentation?`.
    fn special_inner_call_indentation(&self, ctx: &Context<'_>, call_span: Span) -> bool {
        match self.style {
            Style::Consistent => false,
            Style::ConsistentRelativeToReceiver => true,
            Style::SpecialForInnerMethodCall | Style::SpecialForInnerMethodCallInParentheses => {
                let Some(parent) = logical_parent(ctx) else { return false };
                if parent.kind != NodeKind::CallNode {
                    return false;
                }
                let Some(facts) = self.facts.last() else { return false };
                if facts.is_safe_navigation || facts.is_index_setter {
                    return false;
                }
                if !facts.parenthesized
                    && self.style == Style::SpecialForInnerMethodCallInParentheses
                {
                    return false;
                }
                call_span.start > parent.span.start
            }
        }
    }

    /// RuboCop's `base_range`'s `start_node`: the call's immediate parent when it is a
    /// `*`/`**` splat wrapping the call (`foo(*bar(\n  x))`), else the call itself.
    fn base_range_start(ctx: &Context<'_>, call_span: Span) -> u32 {
        match ctx.parent() {
            Some(p) if matches!(p.kind, NodeKind::SplatNode | NodeKind::AssocSplatNode) => {
                p.span.start
            }
            _ => call_span.start,
        }
    }

    /// RuboCop's `column_of`.
    #[allow(clippy::naive_bytecount)]
    fn column_of(&mut self, ctx: &Context<'_>, start: u32, arg_start: u32) -> i64 {
        let stripped = ruby_strip(ctx.text(Span::new(start, arg_start)));
        if stripped.contains(&b'\n') {
            let newlines =
                u32::try_from(stripped.iter().filter(|&&b| b == b'\n').count()).unwrap_or(u32::MAX);
            let target = ctx.line_col(start).line + newlines + 1;
            i64::from(self.previous_code_line_indent(ctx, target))
        } else {
            i64::from(ctx.display_column(start))
        }
    }

    /// RuboCop's `previous_code_line`, returning the found line's `=~ /\S/` column directly
    /// (the only thing either call site uses the line for).
    fn previous_code_line_indent(&mut self, ctx: &Context<'_>, mut line: u32) -> u32 {
        let comment_lines = self.comment_lines.get_or_insert_with(|| {
            ctx.comments().iter().filter(|c| begins_its_line(ctx, c.span)).map(|c| c.line).collect()
        });
        loop {
            if line <= 1 {
                return 0;
            }
            line -= 1;
            let text = ctx.line_text(line);
            if text.iter().all(|&b| is_ruby_ws_byte(b)) || comment_lines.contains(&line) {
                continue;
            }
            return char_indent_of(text);
        }
    }

    /// RuboCop's `message`.
    fn build_message(ctx: &Context<'_>, start: u32, arg_start: u32, special: bool) -> String {
        let stripped = ruby_strip(ctx.text(Span::new(start, arg_start)));
        let base = if !stripped.contains(&b'\n') && special {
            format!("`{}`", String::from_utf8_lossy(stripped))
        } else {
            let last_line = stripped.rsplit(|&b| b == b'\n').next().unwrap_or(stripped);
            if is_comment_only_line(last_line) {
                "the start of the previous line (not counting the comment)".to_string()
            } else {
                "the start of the previous line".to_string()
            }
        };
        format!("Indent the first argument one step more than {base}.")
    }
}

/// RuboCop's `node.parent`, bridged across Prism's `ArgumentsNode` wrapper: a call argument's
/// immediate Prism parent is the `ArgumentsNode` holding the whole argument list, whereas
/// whitequark (RuboCop's own AST) has arguments as direct children of the call node itself. When
/// the immediate parent is that wrapper, the node RuboCop means is one level further up.
fn logical_parent(ctx: &Context<'_>) -> Option<NodeInfo> {
    let ancestors = ctx.ancestors();
    let immediate = *ancestors.last()?;
    if immediate.kind == NodeKind::ArgumentsNode {
        ancestors.get(ancestors.len().checked_sub(2)?).copied()
    } else {
        Some(immediate)
    }
}

/// RuboCop-AST's `OPERATOR_METHODS`.
fn is_operator_method(name: &[u8]) -> bool {
    matches!(
        name,
        b"|" | b"^"
            | b"&"
            | b"<=>"
            | b"=="
            | b"==="
            | b"=~"
            | b">"
            | b">="
            | b"<"
            | b"<="
            | b"<<"
            | b">>"
            | b"+"
            | b"-"
            | b"*"
            | b"/"
            | b"%"
            | b"**"
            | b"~"
            | b"+@"
            | b"-@"
            | b"!@"
            | b"~@"
            | b"[]"
            | b"[]="
            | b"!"
            | b"!="
            | b"!~"
            | b"`"
    )
}

fn facts_of_call(call: &CallNode<'_>) -> CallFacts {
    CallFacts {
        is_safe_navigation: call.is_safe_navigation(),
        is_index_setter: call.name().as_slice() == b"[]=",
        parenthesized: call.closing_loc().is_some_and(|l| l.as_slice() == b")"),
    }
}

fn same_line(ctx: &Context<'_>, a: Span, b: Span) -> bool {
    ctx.line_col(a.start).line == ctx.line_col(b.start).line
}

/// RuboCop's `Util#begins_its_line?`, character-based so it also holds on lines with non-ASCII
/// leading content.
fn begins_its_line(ctx: &Context<'_>, span: Span) -> bool {
    let line_col = ctx.line_col(span.start);
    let line = ctx.line_text(line_col.line);
    let Ok(text) = std::str::from_utf8(line) else { return line_col.column == 0 };
    match text.chars().position(|ch| !is_ruby_ws_char(ch)) {
        Some(index) => u32::try_from(index).unwrap_or(u32::MAX) == line_col.column,
        None => false,
    }
}

/// Ruby's `\s` character class (used by the cop's `=~ /\S/` column search and by
/// `comment_line?`'s `/^\s*#/`).
fn is_ruby_ws_byte(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

fn is_ruby_ws_char(ch: char) -> bool {
    ch.is_ascii() && is_ruby_ws_byte(ch as u8)
}

/// Ruby's `String#strip`: trims leading/trailing `\0`, `\t`, `\n`, `\v`, `\f`, `\r`, and space.
fn ruby_strip(bytes: &[u8]) -> &[u8] {
    let is_ws = |b: u8| matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r' | 0);
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && is_ws(bytes[start]) {
        start += 1;
    }
    while end > start && is_ws(bytes[end - 1]) {
        end -= 1;
    }
    &bytes[start..end]
}

/// The character index of the first non-whitespace byte in `text` (RuboCop's `=~ /\S/`).
fn char_indent_of(text: &[u8]) -> u32 {
    match std::str::from_utf8(text) {
        Ok(s) => u32::try_from(s.chars().take_while(|&c| is_ruby_ws_char(c)).count()).unwrap_or(0),
        Err(_) => {
            u32::try_from(text.iter().take_while(|&b| is_ruby_ws_byte(*b)).count()).unwrap_or(0)
        }
    }
}

/// RuboCop's `Util#comment_line?`: `/^\s*#/`.
fn is_comment_only_line(line: &[u8]) -> bool {
    let mut i = 0;
    while i < line.len() && is_ruby_ws_byte(line[i]) {
        i += 1;
    }
    i < line.len() && line[i] == b'#'
}

/// Collects the byte ranges of heredoc bodies within a subtree, so autocorrection never touches
/// lines that are really heredoc content (RuboCop's `AlignmentCorrector` `inside_string_ranges`/
/// `inside_string_range`, heredoc case).
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

/// RuboCop's `AlignmentCorrector.correct`: shifts every physical line of `item` by
/// `column_delta` columns. Returns `None` when nothing could be safely edited (a `=begin`/
/// `=end` block comment inside the range, or every line was blocked by a taboo range/whitespace
/// mismatch).
fn build_shift_fix(ctx: &Context<'_>, item: &Node<'_>, column_delta: i64) -> Option<Fix> {
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

impl Rule for FirstArgumentIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/FirstArgumentIndentation",
        department: Department::Layout,
        summary: "Checks the indentation of the first argument in a method call.",
        explanation: "\
Arguments after the first one are checked by `Layout/ArgumentAlignment`, not by this cop. For
indenting the first parameter of method _definitions_, check out `Layout/FirstParameterIndentation`.

This cop will respect `Layout/ArgumentAlignment` and will not work when `EnforcedStyle:
with_fixed_indentation` is specified for `Layout/ArgumentAlignment`.

```ruby
# bad
some_method(
first_param,
second_param)

# good (EnforcedStyle: special_for_inner_method_call_in_parentheses, the default)
some_method(
  first_param,
second_param)

# good (EnforcedStyle: consistent)
some_method(
  first_param,
second_param)

# good (EnforcedStyle: consistent_relative_to_receiver)
foo = some_method(
        first_param,
second_param)

# good (EnforcedStyle: special_for_inner_method_call)
some_method(
  first_param,
second_param)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode, NodeKind::SuperNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("special_for_inner_method_call_in_parentheses"),
                allowed: &[
                    "consistent",
                    "consistent_relative_to_receiver",
                    "special_for_inner_method_call",
                    "special_for_inner_method_call_in_parentheses",
                ],
                doc: "The indentation style for the first argument of a multi-line method call.",
            },
            ConfigOption {
                name: "IndentationWidth",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "Overrides `Layout/IndentationWidth`'s configured width for this cop alone.",
            },
        ],
        blind_spots: "\
`enforce_first_argument_with_fixed_indentation?`/`enable_layout_first_method_argument_line_break?`
read `Layout/ArgumentAlignment`'s `EnforcedStyle` and `Layout/FirstMethodArgumentLineBreak`'s
`Enabled` through `peer(...)`, which only reflects an explicit key in that cop's own config block
in the loaded YAML, not a cop-wide default or `--only`/`--except` override; a file that relies on
either default to disable this cop is not detected (false negative only: this cop will still run
when RuboCop itself would have skipped it). `AlignmentCorrector`'s non-heredoc delimited-string
taboo ranges (plain
multi-line string/symbol literals) are not tracked, only heredoc bodies -- unlikely to matter for
a first-argument shift, since the argument being corrected is not itself one of those literals in
any fixture case.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "consistent" => Style::Consistent,
            "consistent_relative_to_receiver" => Style::ConsistentRelativeToReceiver,
            "special_for_inner_method_call" => Style::SpecialForInnerMethodCall,
            _ => Style::SpecialForInnerMethodCallInParentheses,
        };
        let width = options
            .get("IndentationWidth")
            .and_then(OptionValue::as_int)
            .or_else(|| {
                options.peer("Layout/IndentationWidth", "Width").and_then(OptionValue::as_int)
            })
            .unwrap_or(2);
        let fixed_indentation =
            options.peer("Layout/ArgumentAlignment", "EnforcedStyle").and_then(OptionValue::as_str)
                == Some("with_fixed_indentation");
        let line_break_enabled = options
            .peer("Layout/FirstMethodArgumentLineBreak", "Enabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(true);
        Ok(Self {
            style,
            width,
            skip_with_fixed_indentation: fixed_indentation && !line_break_enabled,
            facts: Vec::new(),
            reported: Vec::new(),
            comment_lines: None,
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.facts.clear();
        self.reported.clear();
        self.comment_lines = None;
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                if let Some((span, first_arg)) = Self::call_target(&call) {
                    self.check(ctx, span, &first_arg);
                }
                self.facts.push(facts_of_call(&call));
            }
            Node::SuperNode { .. } => {
                let sup = node.as_super_node().expect("kind matched");
                if let Some((span, first_arg)) = Self::super_target(&sup) {
                    self.check(ctx, span, &first_arg);
                }
                self.facts.push(CallFacts::default());
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if matches!(node, Node::CallNode { .. } | Node::SuperNode { .. }) {
            self.facts.pop();
        }
    }
}
