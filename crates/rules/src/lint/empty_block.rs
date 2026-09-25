//! `Lint/EmptyBlock`, ported from RuboCop's `lib/rubocop/cop/lint/empty_block.rb`.
//!
//! RuboCop's `on_block` fires for whitequark's `block`/`numblock`/`itblock`
//! node types, whose own source range always starts at the dispatched call
//! (`items.each { }`, not just `{ }`) since whitequark nests the `send`
//! inside the block node. Prism instead attaches a literal block directly to
//! the [`ruby_ast::node::CallNode`]/[`ruby_ast::node::SuperNode`]/
//! [`ruby_ast::node::ForwardingSuperNode`] it belongs to via a `block` field,
//! and a `CallNode`'s (and friends') own span already extends through that
//! attached block -- so subscribing to those four node kinds directly (a
//! stabby lambda `->() { }` is its own [`ruby_ast::node::LambdaNode`], never
//! a `CallNode`) and reporting each one's own [`ruby_ast::NodeExt::span`]
//! reproduces RuboCop's offense range without any ancestor lookup, exactly
//! like `Style/SymbolProc`'s dispatch shape.
//!
//! `on_numblock`/`on_itblock` are deliberately left unhandled upstream (the
//! cop disables `InternalAffairs/NumblockHandler` for it): a numbered- or
//! `it`-parameter block only gets Parser's `numblock`/`itblock` node type
//! when the body actually references `_1`/`it`, which an empty body never
//! does. Prism mirrors this exactly -- [`ruby_ast::node::BlockNode::parameters`]
//! only yields a `NumberedParametersNode`/`ItParametersNode` when the (then
//! non-empty) body uses them -- so checking `body().is_none()` alone already
//! excludes every numbered-/`it`-parameter block from ever matching here.
//!
//! RuboCop's `allow_comment?` first checks `contains_comment?` (any comment
//! on any line the node's own range spans, regardless of column), then
//! special-cases a comment on the node's own first line that is itself a
//! `# rubocop:disable`/`todo` directive for this cop (that comment does not
//! count as an "explanatory" comment, so it does not suppress via this
//! path) -- but such a comment is dropped anyway by the engine's own
//! directive handling once this rule reports it, so the net visible
//! diagnostics are identical whether or not that one sub-case is
//! special-cased. [`allow_comment`] therefore only ports the simpler
//! "any comment on any spanned line" check.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Empty block detected.";

/// RuboCop's `contains_comment?`: any comment on any line `span` spans
/// (inclusive of both endpoints), regardless of column.
fn allow_comment(ctx: &Context<'_>, span: Span) -> bool {
    let start_line = ctx.line_col(span.start).line;
    let end_line = ctx.line_col(span.end.saturating_sub(1)).line;
    ctx.comments().iter().any(|c| c.line >= start_line && c.line <= end_line)
}

/// Checks for blocks without a body. Empty lambdas and procs are ignored
/// by default.
#[derive(Debug, Clone)]
pub struct EmptyBlock {
    allow_comments: bool,
    allow_empty_lambdas: bool,
}

impl EmptyBlock {
    /// RuboCop's `on_block` tail, once the node's own attached block/lambda
    /// body has already been confirmed empty: `allow_empty_lambdas?`, then
    /// `AllowComments`, then report.
    fn check(&self, ctx: &mut Context<'_>, span: Span, lambda_or_proc: bool) {
        if self.allow_empty_lambdas && lambda_or_proc {
            return;
        }
        if self.allow_comments && allow_comment(ctx, span) {
            return;
        }
        ctx.report(&Self::META, span, MSG);
    }

    fn check_call(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        let Some(block_node) = call.block() else { return };
        let Node::BlockNode { .. } = &block_node else { return };
        let block = block_node.as_block_node().expect("kind matched");
        if block.body().is_some() {
            return;
        }
        self.check(ctx, node.span(), ext::is_lambda_or_proc(&call));
    }

    fn check_super(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let sup = node.as_super_node().expect("kind matched");
        let Some(block_node) = sup.block() else { return };
        let Node::BlockNode { .. } = &block_node else { return };
        let block = block_node.as_block_node().expect("kind matched");
        if block.body().is_some() {
            return;
        }
        self.check(ctx, node.span(), false);
    }

    fn check_forwarding_super(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let sup = node.as_forwarding_super_node().expect("kind matched");
        let Some(block) = sup.block() else { return };
        if block.body().is_some() {
            return;
        }
        self.check(ctx, node.span(), false);
    }

    fn check_lambda(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let lambda = node.as_lambda_node().expect("kind matched");
        if lambda.body().is_some() {
            return;
        }
        // A `->` literal is always a lambda: `allow_empty_lambdas?` alone
        // decides it, unconditionally.
        self.check(ctx, node.span(), true);
    }
}

impl Rule for EmptyBlock {
    const META: RuleMeta = RuleMeta {
        name: "Lint/EmptyBlock",
        department: Department::Lint,
        summary: "Checks for blocks without a body.",
        explanation: "\
Such empty blocks are typically an oversight or we should provide a comment
to clarify what we're aiming for.

Empty lambdas and procs are ignored by default.

NOTE: For backwards compatibility, the configuration that allows/disallows
empty lambdas and procs is called `AllowEmptyLambdas`, even though it also
applies to procs.

```ruby
# bad
items.each { |item| }

# good
items.each { |item| puts item }
```

With `AllowComments: true` (default), a block/lambda with a comment inside
it or trailing it on the same line is left alone:

```ruby
# good
items.each do |item|
  # TODO: implement later (inner comment)
end

items.each { |item| } # TODO: implement later (inline comment)
```

With `AllowEmptyLambdas: true` (default), `-> { }`, `lambda do end`,
`proc { }`, and `Proc.new { }` are all left alone.",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::SuperNode,
            NodeKind::ForwardingSuperNode,
            NodeKind::LambdaNode,
        ],
        config: &[
            ConfigOption {
                name: "AllowComments",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow a block/lambda with a comment inside or trailing it.",
            },
            ConfigOption {
                name: "AllowEmptyLambdas",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow an empty `->`/`lambda`/`proc`/`Proc.new`.",
            },
        ],
        blind_spots: "\
`comment_disables_cop?` (a `# rubocop:disable`/`todo` comment for this cop
specifically, on the node's own first line, does not itself count as an
`AllowComments` explanation) is not special-cased: the engine's own
directive handling drops that diagnostic anyway once reported, so the final
diagnostics are identical either way.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_comments: options.bool("AllowComments"),
            allow_empty_lambdas: options.bool("AllowEmptyLambdas"),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::CallNode { .. } => self.check_call(node, ctx),
            Node::SuperNode { .. } => self.check_super(node, ctx),
            Node::ForwardingSuperNode { .. } => self.check_forwarding_super(node, ctx),
            Node::LambdaNode { .. } => self.check_lambda(node, ctx),
            _ => {}
        }
    }
}
