//! `Layout/DotPosition`, ported from RuboCop's
//! `lib/rubocop/cop/layout/dot_position.rb`.
//!
//! `on_send`/`on_csend` collapse into one `enter` over `NodeKind::CallNode`:
//! Prism gives every method dispatch -- with or without a connecting dot,
//! plain or safe-navigation -- the same node type, so there is no separate
//! `csend` kind to alias into the same handler. `node.dot?`/`safe_navigation?`
//! become a check of `CallNode::call_operator_loc`'s own text (`.`) or
//! `CallNode::is_safe_navigation` (`&.`); a `::`-joined call (also carried in
//! `call_operator_loc`) is excluded exactly like upstream's `double_colon?`
//! sibling.
//!
//! A heredoc receiver/argument needs its *closing* line, not its node's own
//! span end: per the Prism-vs-whitequark heredoc trap, a heredoc string
//! node's location stops at its opener (`<<~SQL`), matching whitequark's
//! `node.source_range.end` for this same cop's purposes closely enough that
//! `receiver_end_line`/`last_heredoc_line` only need to special-case the
//! heredoc's own `closing_loc` for the *line-gap* check; `autocorrect`'s
//! `insert_after(node.receiver, ...)` still anchors on the receiver's own
//! (opener-only) span end, exactly as upstream's `node.receiver` node does.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// The dot belongs on the next line, with the method name.
    Leading,
    /// The dot belongs on the previous line, with the receiver.
    Trailing,
}

/// Checks the position of the dot in multi-line method calls, ported from
/// RuboCop's `DotPosition` cop.
#[derive(Debug, Clone)]
pub struct DotPosition {
    style: Style,
}

impl Rule for DotPosition {
    const META: RuleMeta = RuleMeta {
        name: "Layout/DotPosition",
        department: Department::Layout,
        summary: "Checks the position of the dot in multi-line method calls.",
        explanation: "\
Checks the `.` position in multi-line method calls.

```ruby
# EnforcedStyle: leading (default)

# bad
something.
  method

# good
something
  .method
```

```ruby
# EnforcedStyle: trailing

# bad
something
  .method

# good
something.
  method
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[linter::ConfigOption {
            name: "EnforcedStyle",
            default: linter::ConfigDefault::Str("leading"),
            allowed: &["leading", "trailing"],
            doc: "Whether the dot connecting a multi-line method call to its \
                  receiver belongs on the next line with the method name \
                  (`leading`) or the previous line with the receiver \
                  (`trailing`).",
        }],
        blind_spots: "\
`self.autocorrect_incompatible_with` (`Style::RedundantSelf`, avoiding a
double-correction clash when both cops run together) is not ported: this
port has no cross-rule autocorrect-conflict mechanism.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "trailing" => Style::Trailing,
            _ => Style::Leading,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Node::CallNode { .. } = node else { return };
        let call = node.as_call_node().expect("kind matched");
        let Some(dot_loc) = call.call_operator_loc() else { return };
        let is_safe_navigation = call.is_safe_navigation();
        let dot_span = dot_loc.span();
        let dot_text = dot_loc.as_slice();
        if dot_text != b"." && !is_safe_navigation {
            // Excludes `::`, matching upstream's `dot? || safe_navigation?`.
            return;
        }
        if self.proper_dot_position(&call, dot_span, ctx) {
            return;
        }
        let message = self.message(dot_text);
        let fix = self.autocorrect(&call, dot_span, dot_text, ctx);
        ctx.report_with_fix(&Self::META, dot_span, message, fix);
    }
}

impl DotPosition {
    /// RuboCop's `message`.
    fn message(&self, dot_text: &[u8]) -> String {
        let dot = String::from_utf8_lossy(dot_text);
        match self.style {
            Style::Leading => {
                format!("Place the {dot} on the next line, together with the method name.")
            }
            Style::Trailing => {
                format!(
                    "Place the {dot} on the previous line, together with the method call receiver."
                )
            }
        }
    }

    /// RuboCop's `proper_dot_position?`.
    fn proper_dot_position(&self, call: &CallNode<'_>, dot_span: Span, ctx: &Context<'_>) -> bool {
        let selector_span = selector_span(call);
        // `receiver` always exists: a dot/`&.` operator implies an explicit
        // receiver by Ruby syntax.
        let receiver = call.receiver().expect("dot call has a receiver");
        let receiver_raw_end = receiver.span().end;
        if ctx.same_line(selector_span, Span::new(receiver_raw_end, receiver_raw_end)) {
            return true;
        }
        let selector_line = ctx.line_col(selector_span.start).line;
        let receiver_line = receiver_end_line(&receiver, ctx);
        let dot_line = ctx.line_col(dot_span.start).line;
        let max_line = receiver_line.max(dot_line);
        if line_between(selector_line, max_line) {
            return true;
        }
        match self.style {
            Style::Leading => dot_line == selector_line,
            Style::Trailing => dot_line != selector_line,
        }
    }

    /// RuboCop's `autocorrect`.
    fn autocorrect(
        &self,
        call: &CallNode<'_>,
        dot_span: Span,
        dot_text: &[u8],
        ctx: &Context<'_>,
    ) -> Fix {
        let dot_line = ctx.line_col(dot_span.start).line;
        let mut edits = Vec::with_capacity(2);
        // RuboCop's `processed_source[dot.line - 1].strip == '.'`: compares
        // the whole line, trimmed, to the literal character `.` -- not to
        // `dot.source`, so a lone `&.` on its own line does not qualify.
        if ctx.line_text(dot_line).trim_ascii() == b"." {
            edits.push(Edit::delete(ctx.whole_lines(dot_span)));
        } else {
            edits.push(Edit::delete(dot_span));
        }
        match self.style {
            Style::Leading => {
                let selector_span = selector_span(call);
                edits.push(Edit::insert(selector_span.start, dot_text.to_vec()));
            }
            Style::Trailing => {
                // `receiver` is guaranteed by the same syntax invariant as in
                // `proper_dot_position?`.
                let receiver = call.receiver().expect("dot call has a receiver");
                edits.push(Edit::insert(receiver.span().end, dot_text.to_vec()));
            }
        }
        Fix { applicability: Applicability::Safe, edits }
    }
}

/// RuboCop's `line_between?`.
fn line_between(first_line: u32, second_line: u32) -> bool {
    first_line > second_line + 1
}

/// RuboCop's `selector_range`, restricted to the `call_type?` branch (this
/// cop only ever calls it with the `CallNode` itself): `node.loc.selector ||
/// node.loc.begin` -- a `l.(1)`-style call with no method name falls back to
/// the opening parenthesis of its argument list.
fn selector_span(call: &CallNode<'_>) -> Span {
    call.message_loc()
        .map(|loc| loc.span())
        .or_else(|| call.opening_loc().map(|loc| loc.span()))
        .unwrap_or_else(|| call.as_node().span())
}

/// RuboCop's `receiver_end_line`.
fn receiver_end_line(receiver: &Node<'_>, ctx: &Context<'_>) -> u32 {
    match last_heredoc_line(receiver, ctx) {
        Some(line) => line,
        None => ctx.line_col(receiver.span().end).line,
    }
}

/// RuboCop's `last_heredoc_line`.
fn last_heredoc_line(node: &Node<'_>, ctx: &Context<'_>) -> Option<u32> {
    if let Node::CallNode { .. } = node {
        let call = node.as_call_node()?;
        let args = call.arguments()?;
        return args
            .arguments()
            .iter()
            .filter_map(|arg| heredoc_closing_span(&arg))
            .map(|span| ctx.line_col(span.start).line)
            .max();
    }
    heredoc_closing_span(node).map(|span| ctx.line_col(span.start).line)
}

/// RuboCop's `heredoc?(node) = node.any_str_type? && node.heredoc?`, fused
/// with reading `node.loc.heredoc_end`: the closing-terminator location of
/// a heredoc string/xstring literal, or `None` for anything else (including
/// a non-heredoc string of the same node kinds).
fn heredoc_closing_span(node: &Node<'_>) -> Option<Span> {
    if !ruby_ast::ext::is_heredoc(node) {
        return None;
    }
    match node {
        Node::StringNode { .. } => {
            node.as_string_node().and_then(|n| n.closing_loc()).map(|l| l.span())
        }
        Node::InterpolatedStringNode { .. } => {
            node.as_interpolated_string_node().and_then(|n| n.closing_loc()).map(|l| l.span())
        }
        Node::XStringNode { .. } => node.as_x_string_node().map(|n| n.closing_loc().span()),
        Node::InterpolatedXStringNode { .. } => {
            node.as_interpolated_x_string_node().map(|n| n.closing_loc().span())
        }
        _ => None,
    }
}
