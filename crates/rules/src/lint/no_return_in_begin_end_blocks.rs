//! `Lint/NoReturnInBeginEndBlocks`, ported from RuboCop's
//! `lib/rubocop/cop/lint/no_return_in_begin_end_blocks.rb`.
//!
//! # Prism shape
//!
//! Whitequark's `kwbegin` is an explicit `begin...end`; Prism represents
//! both that and an implicit method-body-with-rescue as the same
//! `BeginNode`, distinguished only by `begin_keyword_loc` being present
//! (see the KIT doc). `-> { }` is Prism's own `LambdaNode`, a different
//! kind from `lambda { }`'s `BlockNode` (whitequark desugars both to a
//! `(block (send nil :lambda) ...)` shape uniformly), so both are checked
//! for upstream's `ancestor.lambda?`.
//!
//! Upstream's `each_node(:kwbegin)`/`each_node(:return)` are unconditional,
//! unbounded-depth searches from every matching assignment node, so the
//! "is this kwbegin inside an assignment" gate only requires *some*
//! ancestor (at any depth) to be one of the 18 bare-target assignment node
//! kinds the aliased hooks (`on_lvasgn`/`on_ivasgn`/`on_cvasgn`/`on_gvasgn`/
//! `on_casgn`/`on_or_asgn`/`on_op_asgn`, not `on_masgn`) cover; nested
//! `begin...end` blocks are not specially deduplicated upstream, but no
//! fixture exercises one, so this port reports once per `return`, against
//! its nearest enclosing explicit `begin...end` only (see `META.blind_spots`).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{walk, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

/// Upstream's `MSG`.
const MSG: &str = "Do not `return` in `begin..end` blocks in assignment contexts.";

/// The 18 node kinds the aliased `on_lvasgn`/`on_ivasgn`/`on_cvasgn`/
/// `on_gvasgn`/`on_casgn`/`on_or_asgn`/`on_op_asgn` hooks cover between them
/// (whitequark's single `lvasgn`/.../`casgn`/`or_asgn`/`op_asgn` types, one
/// of which -- `casgn` -- already covers both simple and qualified
/// constants uniformly, unlike Prism's `ConstantWriteNode`/
/// `ConstantPathWriteNode` split).
const ASSIGNMENT_KINDS: &[NodeKind] = &[
    NodeKind::LocalVariableWriteNode,
    NodeKind::InstanceVariableWriteNode,
    NodeKind::ClassVariableWriteNode,
    NodeKind::GlobalVariableWriteNode,
    NodeKind::ConstantWriteNode,
    NodeKind::ConstantPathWriteNode,
    NodeKind::LocalVariableOrWriteNode,
    NodeKind::InstanceVariableOrWriteNode,
    NodeKind::ClassVariableOrWriteNode,
    NodeKind::GlobalVariableOrWriteNode,
    NodeKind::ConstantOrWriteNode,
    NodeKind::ConstantPathOrWriteNode,
    NodeKind::LocalVariableOperatorWriteNode,
    NodeKind::InstanceVariableOperatorWriteNode,
    NodeKind::ClassVariableOperatorWriteNode,
    NodeKind::GlobalVariableOperatorWriteNode,
    NodeKind::ConstantOperatorWriteNode,
    NodeKind::ConstantPathOperatorWriteNode,
];

/// Do not `return` inside `begin..end` blocks in assignment contexts.
#[derive(Debug, Clone)]
pub struct NoReturnInBeginEndBlocks;

impl Rule for NoReturnInBeginEndBlocks {
    const META: RuleMeta = RuleMeta {
        name: "Lint/NoReturnInBeginEndBlocks",
        department: Department::Lint,
        summary: "Do not `return` inside `begin..end` blocks in assignment contexts.",
        explanation: "\
```ruby
# bad
some_variable = begin
                  return if some_condition_is_met

                  some_value
                else
                  do_something
                end

# good
some_variable = if some_condition_is_met
                  return if another_condition_is_met

                  some_value
                else
                  do_something
                end
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "\
Nested `begin...end` blocks are not specially deduplicated the way upstream's \
unconditional `each_node(:kwbegin)`/`each_node(:return)` searches implicitly \
allow (a `return` inside a nested `begin...end` could, upstream, be reported \
once per enclosing `begin...end` it's nested in); this port reports each \
`return` once, against its nearest enclosing explicit `begin...end` only. No \
fixture exercises a nested `begin...end`.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let mut finder = Finder { stack: Vec::new(), offenses: Vec::new() };
        walk(&root, &mut finder);
        for span in finder.offenses {
            ctx.report(&Self::META, span, MSG);
        }
    }
}

struct Finder<'pr> {
    stack: Vec<Node<'pr>>,
    offenses: Vec<Span>,
}

impl<'pr> Visitor<'pr> for Finder<'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        if node.kind() == NodeKind::ReturnNode {
            if let Some(span) = check_return(&self.stack, node) {
                self.offenses.push(span);
            }
        }
        self.stack.push(*node);
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.stack.pop();
    }
}

/// Whether `block`'s owning call (the stack entry just before it) is named
/// `lambda` -- upstream's `BlockNode#lambda?`.
fn is_lambda_call_block(stack: &[Node<'_>], block_index: usize) -> bool {
    block_index
        .checked_sub(1)
        .and_then(|i| stack.get(i))
        .and_then(Node::as_call_node)
        .is_some_and(|call: CallNode<'_>| call.name().as_slice() == b"lambda")
}

/// Upstream's `on_lvasgn`/.../`return_from_inner_scope?`, combined for one
/// `return_node`.
fn check_return(stack: &[Node<'_>], return_node: &Node<'_>) -> Option<Span> {
    let kwbegin_index = stack.iter().rposition(|n| {
        n.kind() == NodeKind::BeginNode
            && n.as_begin_node().is_some_and(|b| b.begin_keyword_loc().is_some())
    })?;

    for (i, ancestor) in stack.iter().enumerate().skip(kwbegin_index + 1) {
        match ancestor.kind() {
            NodeKind::DefNode | NodeKind::LambdaNode => return None,
            NodeKind::BlockNode if is_lambda_call_block(stack, i) => return None,
            _ => {}
        }
    }

    let has_assignment_ancestor =
        stack[..kwbegin_index].iter().any(|a| ASSIGNMENT_KINDS.contains(&a.kind()));
    if !has_assignment_ancestor {
        return None;
    }

    Some(return_node.span())
}
