//! `Layout/EmptyLinesAroundBlockBody`, ported from RuboCop's
//! `lib/rubocop/cop/layout/empty_lines_around_block_body.rb` plus the
//! `EmptyLinesAroundBody` mixin it includes (shared logic lives in
//! `empty_lines_around_body.rs`).
//!
//! Whitequark wraps a call plus its literal block in one `:block` node whose
//! own source range covers the call too; the call itself is not the block
//! body's ancestor. Prism is the reverse: [`ruby_ast::node::CallNode`] (and
//! [`ruby_ast::node::SuperNode`]/[`ruby_ast::node::ForwardingSuperNode`])
//! owns the block via a `block` field, and that outer node's own span
//! already extends through the attached block -- so this rule subscribes to
//! those three kinds directly and reads back their `block()` field (`&blk`,
//! a [`ruby_ast::node::BlockArgumentNode`], is filtered out by
//! `as_block_node`). RuboCop's `node.send_node.last_line` (the adjusted
//! first line, needed when the call's own arguments span multiple lines
//! before the block opens) becomes the block's own opening delimiter
//! (`do`/`{`) line, always on the same physical line as the call's tail.
//!
//! `rubocop-ast`'s `BlockNode#single_line?` is overridden to compare only
//! the opening/closing delimiter lines (`do`/`end` or `{`/`}`), not the
//! whole node's own first/last line the generic `Node#single_line?` uses --
//! otherwise a block whose *receiver* spans multiple lines (e.g. a
//! multi-line `%w(...)` array literal) but whose own body sits entirely on
//! one line would wrongly count as multiline. This port replicates that by
//! comparing the opening/closing delimiter lines directly instead of
//! gating on the whole node's span.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

use super::empty_lines_around_body::{check_both, Want};

/// Keeps track of empty lines around block bodies.
#[derive(Debug, Clone)]
pub struct EmptyLinesAroundBlockBody {
    want: Want,
}

impl Rule for EmptyLinesAroundBlockBody {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLinesAroundBlockBody",
        department: Department::Layout,
        summary: "Keeps track of empty lines around block bodies.",
        explanation: "\
```ruby
# EnforcedStyle: no_empty_lines (default)

# bad
foo do |bar|

  # ...

end

# good
foo do |bar|
  # ...
end
```

```ruby
# EnforcedStyle: empty_lines

# bad
foo do |bar|
  # ...
end

# good
foo do |bar|

  # ...

end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::SuperNode, NodeKind::ForwardingSuperNode],
        config: &[linter::ConfigOption {
            name: "EnforcedStyle",
            default: linter::ConfigDefault::Str("no_empty_lines"),
            allowed: &["empty_lines", "no_empty_lines"],
            doc: "The blank-line convention required around a block body.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let want = match options.style("EnforcedStyle")? {
            "empty_lines" => Want::Empty,
            _ => Want::NoEmpty,
        };
        Ok(Self { want })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let block = match node {
            Node::CallNode { .. } => node.as_call_node().expect("kind matched").block(),
            Node::SuperNode { .. } => node.as_super_node().expect("kind matched").block(),
            Node::ForwardingSuperNode { .. } => {
                node.as_forwarding_super_node().expect("kind matched").block().map(|b| b.as_node())
            }
            _ => return,
        };
        let Some(block) = block.and_then(|b| b.as_block_node()) else { return };

        // RuboCop's `valid_body_style?`: an empty body is never flagged
        // unless the style is `no_empty_lines` (which forbids a blank line
        // even inside an otherwise-empty body).
        let body = block.body();
        if body.is_none() && self.want != Want::NoEmpty {
            return;
        }

        let first_line = ctx.line_col(block.opening_loc().span().start).line;
        let last_line = ctx.line_col(block.closing_loc().span().start).line;
        // `rubocop-ast`'s `BlockNode#single_line?` is overridden to compare
        // only the opening/closing delimiter lines (`do`/`end` or `{`/`}`),
        // not the whole node's own first/last line -- which would
        // (wrongly) count a block whose *receiver* spans multiple lines
        // (e.g. a multi-line `%w(...)` array) as multiline even when its
        // own body sits entirely on one line.
        if first_line == last_line {
            return;
        }

        check_both(ctx, &Self::META, "block", self.want, first_line, last_line);
    }
}
