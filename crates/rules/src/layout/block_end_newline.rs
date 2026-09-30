//! `Layout/BlockEndNewline`, ported from RuboCop's
//! `lib/rubocop/cop/layout/block_end_newline.rb` plus the `Alignment` mixin
//! it includes (only for `begins_its_line?`, reached here through
//! [`Context::begins_its_line`]).
//!
//! Prism's `BlockNode` covers whitequark's `block`/`numblock`/`itblock`
//! trio in one shape (a numbered (`_1`) or `it` implicit parameter never
//! gets its own parameter-list node, just an ordinary body reference), so
//! this rule only needs the one [`NodeKind::BlockNode`] subscription where
//! RuboCop aliases `on_numblock`/`on_itblock` to `on_block`.
//!
//! RuboCop's `node.children.compact.last` (whitequark's `(block send args
//! body)` triple) is renormalized as [`last_child_end`]: Prism's `BlockNode`
//! only carries `body`/`parameters` directly (the call is the block's
//! *parent*, not a child), so the fallback to "the call node itself" is
//! approximated with the block's own opening keyword -- a state
//! [`Rule::enter`]'s `begins_its_line` guard already makes unreachable in
//! valid Ruby (a body-less, parameter-less multiline block's `end`/`}`
//! always begins its own line).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::BlockNode;
use ruby_ast::{ext::is_heredoc, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Put end statement of multiline block on its own line.
#[derive(Debug, Clone)]
pub struct BlockEndNewline;

impl Rule for BlockEndNewline {
    const META: RuleMeta = RuleMeta {
        name: "Layout/BlockEndNewline",
        department: Department::Layout,
        summary: "Put end statement of multiline block on its own line.",
        explanation: "\
```ruby
# bad
blah do |i|
  foo(i) end

# good
blah do |i|
  foo(i)
end

# bad
blah { |i|
  foo(i) }

# good
blah { |i|
  foo(i)
}
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::BlockNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(block) = node.as_block_node() else { return };
        let span = node.span();
        if ctx.is_single_line(span) {
            return;
        }

        let closing = block.closing_loc().span();
        // RuboCop's `return if begins_its_line?(node.loc.end)`.
        if ctx.begins_its_line(closing) {
            return;
        }

        let offense_span = Span::new(last_child_end(&block), closing.end);
        let offense_text = lstrip(ctx.text(offense_span));
        // RuboCop's `return if offense_range.source.lstrip.start_with?(';')`.
        if offense_text.first() == Some(&b';') {
            return;
        }

        let line_col = ctx.line_col(closing.start);
        let message = format!(
            "Expression at {}, {} should be on its own line.",
            line_col.line,
            line_col.column + 1
        );

        let mut replacement = Vec::with_capacity(offense_text.len() + 1);
        replacement.push(b'\n');
        replacement.extend_from_slice(offense_text);

        let edits = if let Some(closing_start) =
            last_heredoc_argument(effective_body(block.body())).and_then(heredoc_closing_start)
        {
            let closing_line = ctx.line_col(closing_start).line;
            let insert_at = ctx.line_span(closing_line).end;
            vec![Edit::delete(offense_span), Edit::insert(insert_at, replacement)]
        } else {
            vec![Edit::replace(offense_span, replacement)]
        };

        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, closing, message, fix);
    }
}

/// RuboCop's `offense_range`'s base: `node.children.compact.last`'s own end
/// offset (whitequark's `(block send args body)` triple, compacted). See
/// the module doc comment for the fallback's approximation.
fn last_child_end(block: &BlockNode<'_>) -> u32 {
    if let Some(body) = block.body() {
        return body.span().end;
    }
    if let Some(parameters) = block.parameters() {
        return parameters.span().end;
    }
    block.opening_loc().span().end
}

/// Whitequark elides a single-statement body to that statement directly (no
/// wrapping `begin`); Prism always wraps a block body in a
/// [`NodeKind::StatementsNode`], even for one statement. Renormalizes back
/// to whitequark's shape so [`last_heredoc_argument`]'s `call_type?` guard
/// only succeeds where RuboCop's would.
fn effective_body(body: Option<Node<'_>>) -> Option<Node<'_>> {
    let body = body?;
    if let Node::StatementsNode { .. } = body {
        let stmts = body.as_statements_node().expect("kind matched");
        let list = stmts.body();
        if list.len() == 1 {
            return list.first();
        }
    }
    Some(body)
}

/// RuboCop's `last_heredoc_argument`: the last heredoc-shaped argument of
/// `node` if it is (or, failing that, recursively its receiver is) a plain
/// method call ending in one.
fn last_heredoc_argument(node: Option<Node<'_>>) -> Option<Node<'_>> {
    let node = node?;
    let call = node.as_call_node()?;
    if let Some(args) = call.arguments() {
        // `NodeListIter` has no `rev()`; a forward scan keeping the last
        // match is equivalent to `arguments.reverse.detect`.
        let mut heredoc = None;
        for arg in &args.arguments() {
            if is_heredoc(&arg) {
                heredoc = Some(arg);
            }
        }
        if heredoc.is_some() {
            return heredoc;
        }
    }
    last_heredoc_argument(call.receiver())
}

/// The start offset of a heredoc-shaped node's closing-delimiter line
/// (RuboCop's `heredoc.loc.heredoc_end`'s own line, reached via `enter`'s
/// `line_span` rebuild -- Prism's `closing_loc` starts there too, but
/// unlike whitequark's `heredoc_end` also swallows the trailing newline),
/// for any of the four heredoc-carrying string/xstring node kinds.
fn heredoc_closing_start(node: Node<'_>) -> Option<u32> {
    Some(match node {
        Node::StringNode { .. } => node.as_string_node()?.closing_loc()?.span().start,
        Node::InterpolatedStringNode { .. } => {
            node.as_interpolated_string_node()?.closing_loc()?.span().start
        }
        Node::XStringNode { .. } => node.as_x_string_node()?.closing_loc().span().start,
        Node::InterpolatedXStringNode { .. } => {
            node.as_interpolated_x_string_node()?.closing_loc().span().start
        }
        _ => return None,
    })
}

/// Ruby's `String#lstrip`: drops leading ASCII whitespace bytes.
fn lstrip(text: &[u8]) -> &[u8] {
    let start = text.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(text.len());
    &text[start..]
}
