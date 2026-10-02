//! `Style/RedundantEach`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_each.rb`.
//!
//! Upstream's `redundant_each_method` walks from the *first* `each`-family
//! call in a chain up to an *enclosing* ancestor call (`node.each_ancestor
//! (:call).detect { |a| a.receiver == node && ... }`) to find the second,
//! redundant link; since a node can only ever be the literal `receiver`
//! child of its own immediate AST parent, that "ancestor" search can only
//! ever match the immediate parent. This port keeps a self-maintained stack
//! of every `CallNode` ancestor's `(name, selector span, dot span, receiver
//! span)`, pushed on `enter` and popped on `leave`, so the immediate
//! enclosing call's identity is available without a typed-parent accessor
//! (`Context` only exposes an ancestor's *kind* and *span*, not its typed
//! node). [`immediate_outer`] answers both upstream's ancestor-receiver
//! match (`stack.last().receiver_span == Some(self.span())`) and the
//! `node.parent&.call_type?` check [`offense_range`] needs for the plain
//! `each` message, since both are the same "is my own immediate AST parent
//! a call that received me?" question, just asked from two different
//! nodes' perspectives (the inner node finding its outer, and a node
//! checking its own outer for its own range).
//!
//! A `&blk`/`&:sym` block-pass argument is Prism's `CallNode::block()`
//! returning a `BlockArgumentNode` rather than a `BlockNode` (see the
//! porting kit); this port treats that identically to upstream's
//! `last_argument&.block_pass_type?` guard, and a real attached block the
//! same as upstream's `parent&.any_block_type?` (whitequark wraps a
//! block-bearing send in a `block` node, making the send's own AST parent a
//! block; Prism instead keeps it on the same `CallNode` as `block()`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Remove redundant `each`.";
/// RuboCop's `MSG_WITH_INDEX`.
const MSG_WITH_INDEX: &str = "Use `with_index` to remove redundant `each`.";
/// RuboCop's `MSG_WITH_OBJECT`.
const MSG_WITH_OBJECT: &str = "Use `with_object` to remove redundant `each`.";

/// One `CallNode` ancestor's shape, kept on [`RedundantEach`]'s own stack so
/// a descendant can answer "is my immediate AST parent a call, and which
/// one" without a typed-parent accessor.
#[derive(Debug, Clone)]
struct CallInfo {
    name: Vec<u8>,
    receiver_span: Option<Span>,
    dot_span: Option<Span>,
    selector_span: Option<Span>,
}

/// Checks for redundant `each`.
#[derive(Debug, Clone, Default)]
pub struct RedundantEach {
    stack: Vec<CallInfo>,
}

impl Rule for RedundantEach {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantEach",
        department: Department::Style,
        summary: "Checks for redundant `each`.",
        explanation: "\
Checks for redundant `each`.

```ruby
# bad
array.each.each { |v| do_something(v) }

# good
array.each { |v| do_something(v) }

# bad
array.each.each_with_index { |v, i| do_something(v, i) }

# good
array.each.with_index { |v, i| do_something(v, i) }
array.each_with_index { |v, i| do_something(v, i) }

# bad
array.each.each_with_object { |v, o| do_something(v, o) }

# good
array.each.with_object { |v, o| do_something(v, o) }
array.each_with_object { |v, o| do_something(v, o) }
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
This cop is unsafe, as it can produce false positives if the receiver is
not an `Enumerator` (matching upstream's own `@safety` note).",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name().as_slice();
        let receiver_span = call.receiver().as_ref().map(ruby_ast::NodeExt::span);
        let block_kind = call.block().as_ref().map(ruby_ast::NodeExt::kind);
        let has_blocknode = block_kind == Some(NodeKind::BlockNode);
        let has_block_pass = block_kind == Some(NodeKind::BlockArgumentNode);

        if is_restrict(name) {
            self.check(ctx, node, &call, name, has_blocknode, has_block_pass);
        }

        self.stack.push(CallInfo {
            name: name.to_vec(),
            receiver_span,
            dot_span: call.call_operator_loc().map(|l| l.span()),
            selector_span: call.message_loc().map(|l| l.span()),
        });
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.kind() == NodeKind::CallNode {
            self.stack.pop();
        }
    }
}

impl RedundantEach {
    /// RuboCop's `redundant_each_method` plus its `add_offense` call: both
    /// branches (the ancestor-receiver match for a bare `each`, and the
    /// own-receiver match for any of the three restricted methods) live
    /// here since they share `self.stack` and the fix-building logic.
    fn check(
        &self,
        ctx: &mut Context<'_>,
        node: &Node<'_>,
        call: &CallNode<'_>,
        name: &[u8],
        has_blocknode: bool,
        has_block_pass: bool,
    ) {
        if has_block_pass {
            return;
        }

        if name == b"each" && !has_blocknode {
            if let Some(outer) = immediate_outer(&self.stack, node.span()) {
                if is_restrict_or_reverse(&outer.name) {
                    self.register(ctx, node, call, name, &outer.name, outer.selector_span);
                    return;
                }
            }
        }

        let Some(prev) = call.receiver() else { return };
        let Some(prev_call) = prev.as_call_node() else { return };
        let prev_block_kind = prev_call.block().as_ref().map(ruby_ast::NodeExt::kind);
        if matches!(prev_block_kind, Some(NodeKind::BlockNode | NodeKind::BlockArgumentNode)) {
            return;
        }
        let prev_name = prev_call.name().as_slice();
        let detected = name != b"each" && prev_name.starts_with(b"each_");
        if detected || prev_name == b"reverse_each" {
            self.register(
                ctx,
                node,
                call,
                name,
                prev_name,
                prev_call.message_loc().map(|l| l.span()),
            );
        }
    }

    /// RuboCop's `add_offense(range, message: message(node)) { |corrector| ... }`.
    /// `redundant_name`/`redundant_selector` describe whichever call this
    /// offense's removal centers on (the ancestor in the first branch, the
    /// receiver in the second).
    fn register(
        &self,
        ctx: &mut Context<'_>,
        node: &Node<'_>,
        call: &CallNode<'_>,
        name: &[u8],
        redundant_name: &[u8],
        redundant_selector: Option<Span>,
    ) {
        let Some(selector) = call.message_loc().map(|l| l.span()) else { return };
        let message = match name {
            b"each" => MSG,
            b"each_with_index" => MSG_WITH_INDEX,
            b"each_with_object" => MSG_WITH_OBJECT,
            _ => return,
        };

        let range = if name == b"each" {
            match immediate_outer(&self.stack, node.span()) {
                Some(outer) => match outer.dot_span {
                    Some(dot) => selector.join(dot),
                    None => selector,
                },
                None => match call.call_operator_loc().map(|l| l.span()) {
                    Some(dot) => dot.join(selector),
                    None => selector,
                },
            }
        } else {
            selector
        };

        let mut edits = Vec::new();
        match name {
            b"each" => {
                edits.push(Edit::delete(range));
                if let Some(sel) = redundant_selector {
                    match redundant_name {
                        b"each_with_index" => {
                            edits.push(Edit::replace(sel, &b"each.with_index"[..]));
                        }
                        b"each_with_object" => {
                            edits.push(Edit::replace(sel, &b"each.with_object"[..]));
                        }
                        _ => {}
                    }
                }
            }
            b"each_with_index" => edits.push(Edit::replace(selector, &b"with_index"[..])),
            b"each_with_object" => edits.push(Edit::replace(selector, &b"with_object"[..])),
            _ => {}
        }

        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// RuboCop's `RESTRICT_ON_SEND`.
fn is_restrict(name: &[u8]) -> bool {
    matches!(name, b"each" | b"each_with_index" | b"each_with_object")
}

/// `RESTRICT_ON_SEND.include?(ancestor.method_name) || ancestor.method?(:reverse_each)`.
fn is_restrict_or_reverse(name: &[u8]) -> bool {
    is_restrict(name) || name == b"reverse_each"
}

/// The top-of-stack `CallInfo` only if it is genuinely `node_span`'s own
/// immediate AST parent -- i.e. `node_span` is exactly that call's
/// `receiver`. See the module doc for why this single check answers both
/// of upstream's `node.parent&.call_type?`-shaped questions.
fn immediate_outer(stack: &[CallInfo], node_span: Span) -> Option<&CallInfo> {
    stack.last().filter(|outer| outer.receiver_span == Some(node_span))
}
