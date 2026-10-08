//! `Style/AutoResourceCleanup`, ported from RuboCop's
//! `lib/rubocop/cop/style/auto_resource_cleanup.rb`.
//!
//! Upstream matches `(send (const {nil? cbase} {:File :Tempfile}) :open ...)`
//! and skips it (`cleanup?`) when the call carries a block argument
//! (`&:read`) or an attached `do...end`/`{}` block (in whitequark, a call
//! with a block is represented as the call being wrapped by a `block` node,
//! so `node.parent.block_type?` is true; Prism instead exposes this
//! directly as `CallNode::block`). Otherwise it is only an offense when the
//! call's own AST parent is a local variable assignment (`f = File.open(...)`)
//! or there is no parent at all -- i.e. it is the sole top-level statement in
//! the file. Prism always wraps the program body in a `StatementsNode` (see
//! `docs/porting/KIT.md`), so "no parent" is reproduced as: the immediate
//! parent is a `StatementsNode` that is itself a direct child of
//! `ProgramNode` and spans exactly this call (its only statement).

use linter::{
    Context, Department, FixAvailability, NodeInfo, OptionError, Rule, RuleMeta, RuleOptions,
    Severity, Stability,
};
use ruby_ast::ext::is_bare_or_toplevel_const;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Suggests the usage of an auto resource cleanup version of a method (if
/// available).
#[derive(Debug, Clone)]
pub struct AutoResourceCleanup;

impl Rule for AutoResourceCleanup {
    const META: RuleMeta = RuleMeta {
        name: "Style/AutoResourceCleanup",
        department: Department::Style,
        summary:
            "Suggests the usage of an auto resource cleanup version of a method (if available).",
        explanation: "Checks for cases when you could use a block \
accepting version of a method that does automatic resource cleanup.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        if call.name().as_slice() != b"open" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !is_file_or_tempfile_const(&receiver) {
            return;
        }
        if is_cleanup(&call, node.span(), ctx) {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };
        let current = ctx.text(Span::new(receiver.span().start, message_loc.span().end)).to_vec();
        let current = String::from_utf8_lossy(&current).into_owned();
        let message = format!("Use the block version of `{current}`.");
        ctx.report(&Self::META, node.span(), message);
    }
}

/// `(const {nil? cbase} {:File :Tempfile})`.
fn is_file_or_tempfile_const(node: &Node<'_>) -> bool {
    if !is_bare_or_toplevel_const(node) {
        return false;
    }
    let name = match node.kind() {
        NodeKind::ConstantReadNode => {
            Some(node.as_constant_read_node().expect("kind matched").name())
        }
        NodeKind::ConstantPathNode => node.as_constant_path_node().expect("kind matched").name(),
        _ => None,
    };
    name.is_some_and(|name| matches!(name.as_slice(), b"File" | b"Tempfile"))
}

/// Upstream's `cleanup?`.
fn is_cleanup(call: &CallNode<'_>, call_span: Span, ctx: &Context<'_>) -> bool {
    if call.block().is_some() {
        return true;
    }
    let ancestors = ctx.ancestors();
    if is_bare_top_level_statement(ancestors, call_span) {
        return false;
    }
    !matches!(ancestors.last(), Some(NodeInfo { kind: NodeKind::LocalVariableWriteNode, .. }))
}

/// Whitequark's `node.parent` is `nil` only when the node is the sole
/// top-level expression in the whole file; see the module doc.
fn is_bare_top_level_statement(ancestors: &[NodeInfo], call_span: Span) -> bool {
    let [NodeInfo { kind: NodeKind::ProgramNode, .. }, NodeInfo { kind: NodeKind::StatementsNode, span }] =
        ancestors
    else {
        return false;
    };
    *span == call_span
}
