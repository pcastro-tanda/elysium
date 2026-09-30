//! `Layout/ClosingHeredocIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/closing_heredoc_indentation.rb` plus the
//! `Heredoc` mixin it includes.
//!
//! # `argument?`/`chained?` and `find_node_used_heredoc_argument`
//!
//! The cop's fallback basis -- a heredoc argument (or receiver of a chained
//! call) may align with the call that carries it instead of with its own
//! opening line -- needs, for a heredoc node visited bottom-up in RuboCop's
//! traversal, structural facts about its *ancestors* that this engine's
//! single top-down pass cannot look back for (`Context`'s ancestor stack
//! only carries `(kind, span)`, not full nodes).
//!
//! Both facts this cop actually needs are instead captured eagerly, in tree
//! order, while each ancestor `CallNode` is visited itself (mirroring
//! `Layout/FirstHashElementIndentation`'s "eager check" technique):
//!
//! - [`HeredocFact`] (`node.argument?`/`node.chained?`): recorded once per
//!   `CallNode`, keyed by its direct receiver's/arguments' own span, the
//!   moment that call is entered -- long before the engine's traversal
//!   naturally reaches the heredoc itself.
//! - [`CallFrame`] (`find_node_used_heredoc_argument`'s upward walk through
//!   consecutive plain-call parents): a manual stack pushed/popped exactly
//!   at every `CallNode`'s enter/leave. Because a heredoc's direct parent,
//!   if it registered a [`HeredocFact`] for it, is always still open on this
//!   stack when the heredoc is later visited, `self.call_stack.last()` at
//!   that point is guaranteed to be exactly that parent -- no separate
//!   adjacency check is needed. Climbing further up only has to compare
//!   each frame's own `direct_parent_is_call` bit (itself captured from
//!   `Context::parent` at that call's own enter, since intervening
//!   non-`CallNode` ancestors would otherwise make two merely-nested stack
//!   frames look adjacent when they are not) against the next frame's
//!   `is_safe_nav` bit.

use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::heredoc_indentation::HeredocIndentation;

/// `node.argument?`/`node.chained?`, precomputed for a heredoc-shaped node
/// the instant its direct parent `CallNode` is visited (see the module doc
/// comment).
#[derive(Debug, Clone, Copy, Default)]
struct HeredocFact {
    /// RuboCop's `argument?`: `parent.send_type?` (a plain, non-`&.` call)
    /// and this node is one of its direct arguments.
    is_argument: bool,
    /// RuboCop's `chained?`: `parent.call_type?` (any call, `&.` included)
    /// and this node is its direct receiver.
    is_chained: bool,
}

/// One entry of the manual `CallNode` ancestor stack (see the module doc
/// comment).
#[derive(Debug, Clone, Copy)]
struct CallFrame {
    /// `!is_safe_navigation` -- RuboCop's `send_type?`.
    is_plain: bool,
    /// Whether `Context::parent` was this same kind, i.e. this call's own
    /// direct tree parent is itself a `CallNode`.
    direct_parent_is_call: bool,
    /// This call's own span, for [`opening_indentation`] when it turns out
    /// to be `find_node_used_heredoc_argument`'s landing node.
    span_start: u32,
}

/// Checks the indentation of here document closings.
#[derive(Debug, Clone, Default)]
pub struct ClosingHeredocIndentation {
    call_stack: Vec<CallFrame>,
    facts: HashMap<u32, HeredocFact>,
}

impl Rule for ClosingHeredocIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/ClosingHeredocIndentation",
        department: Department::Layout,
        summary: "Checks the indentation of here document closings.",
        explanation: "\
```ruby
# bad
class Foo
  def bar
    <<~SQL
      'Hi'
  SQL
  end
end

# good
class Foo
  def bar
    <<~SQL
      'Hi'
    SQL
  end
end

# bad

# heredoc contents is before closing heredoc.
foo arg,
    <<~EOS
  Hi
    EOS

# good
foo arg,
    <<~EOS
  Hi
EOS

# good
foo arg,
    <<~EOS
      Hi
    EOS
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::StringNode,
            NodeKind::InterpolatedStringNode,
            NodeKind::XStringNode,
            NodeKind::InterpolatedXStringNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Node::CallNode { .. } = node {
            let call = node.as_call_node().expect("kind matched");

            if let Some(receiver) = call.receiver() {
                if ext::is_heredoc(&receiver) {
                    self.facts.entry(receiver.span().start).or_default().is_chained = true;
                }
            }
            if !call.is_safe_navigation() {
                if let Some(args) = call.arguments() {
                    for arg in &args.arguments() {
                        if ext::is_heredoc(&arg) {
                            self.facts.entry(arg.span().start).or_default().is_argument = true;
                        }
                    }
                }
            }

            // Prism wraps a call's arguments in an `ArgumentsNode` that
            // whitequark has no equivalent for; treat it as transparent so
            // an argument call's own true tree parent (skipping the
            // wrapper) is compared against, matching `cur.parent.send_type?`.
            let direct_parent_is_call = match ctx.ancestors() {
                [.., p] if p.kind == NodeKind::CallNode => true,
                [.., grandparent, p] if p.kind == NodeKind::ArgumentsNode => {
                    grandparent.kind == NodeKind::CallNode
                }
                _ => false,
            };
            self.call_stack.push(CallFrame {
                is_plain: !call.is_safe_navigation(),
                direct_parent_is_call,
                span_start: node.span().start,
            });
            return;
        }

        let Some((opening, closing)) = HeredocIndentation::heredoc_locs(node) else { return };
        self.check_heredoc(ctx, node, opening, closing);
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if let Node::CallNode { .. } = node {
            self.call_stack.pop();
        }
    }
}

impl ClosingHeredocIndentation {
    /// RuboCop's `on_heredoc`.
    fn check_heredoc(&self, ctx: &mut Context<'_>, node: &Node<'_>, opening: Span, closing: Span) {
        // RuboCop's `SIMPLE_HEREDOC = '<<'`: neither `<<~` nor `<<-`.
        let opening_text = ctx.text(opening);
        if opening_text.get(2) != Some(&b'~') && opening_text.get(2) != Some(&b'-') {
            return;
        }

        let opening_line = ctx.line_col(opening.start).line;
        let opening_indent = indent_level(ctx.line_text(opening_line));

        // Prism's `closing_loc` starts at the closing delimiter line's own
        // start (like whitequark's `heredoc_end`) but also swallows its
        // trailing newline; rebuild the line-only span via `line_span`.
        let closing_line = ctx.line_col(closing.start).line;
        let closing_span = ctx.line_span(closing_line);
        let closing_indent = indent_level(ctx.text(closing_span));

        if opening_indent == closing_indent {
            return;
        }
        if self.argument_indentation_correct(ctx, node, closing_indent) {
            return;
        }

        let is_argument = self.facts.get(&node.span().start).is_some_and(|f| f.is_argument);
        let opening_stripped = String::from_utf8_lossy(strip(ctx.line_text(opening_line)));
        let closing_stripped = String::from_utf8_lossy(strip(ctx.text(closing_span)));
        let message = if is_argument {
            format!(
                "`{closing_stripped}` is not aligned with `{opening_stripped}` or beginning of \
                 method definition."
            )
        } else {
            format!("`{closing_stripped}` is not aligned with `{opening_stripped}`.")
        };

        let closing_text = ctx.text(closing_span);
        let skip = usize::try_from(closing_indent).unwrap_or(closing_text.len());
        let mut indented_end = vec![b' '; usize::try_from(opening_indent).unwrap_or(0)];
        indented_end.extend_from_slice(closing_text.get(skip..).unwrap_or(&[]));

        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(closing_span, indented_end)],
        };
        ctx.report_with_fix(&Self::META, closing_span, message, fix);
    }

    /// RuboCop's `argument_indentation_correct?`.
    fn argument_indentation_correct(
        &self,
        ctx: &Context<'_>,
        node: &Node<'_>,
        closing_indent: u32,
    ) -> bool {
        let fact = self.facts.get(&node.span().start).copied().unwrap_or_default();
        if !fact.is_argument && !fact.is_chained {
            return false;
        }
        let Some(mut i) = self.call_stack.len().checked_sub(1) else { return false };
        loop {
            let frame = self.call_stack[i];
            if frame.direct_parent_is_call {
                let parent = self.call_stack[i - 1];
                if parent.is_plain {
                    i -= 1;
                    continue;
                }
            }
            break;
        }
        let landing_line = ctx.line_col(self.call_stack[i].span_start).line;
        let landing_indent = indent_level(ctx.line_text(landing_line));
        landing_indent == closing_indent
    }
}

/// `ClosingHeredocIndentation`'s own private `indent_level`, distinct from
/// the `Heredoc` mixin's: just `source_line[/\A */].length`, the leading
/// literal-space run of one physical line (no multi-line minimum, no
/// blank-line handling).
fn indent_level(line: &[u8]) -> u32 {
    let mut n = 0u32;
    for &b in line {
        if b == b' ' {
            n += 1;
        } else {
            break;
        }
    }
    n
}

/// Ruby's `String#strip`: drops leading and trailing ASCII whitespace.
fn strip(text: &[u8]) -> &[u8] {
    let start = text.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(text.len());
    let end = text.iter().rposition(|b| !b.is_ascii_whitespace()).map_or(start, |i| i + 1);
    &text[start..end]
}
