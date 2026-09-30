//! RuboCop's `EndKeywordAlignment` mixin
//! (`lib/rubocop/cop/mixin/end_keyword_alignment.rb`), the `CheckAssignment`
//! mixin (`lib/rubocop/cop/mixin/check_assignment.rb`) and the two
//! `AlignmentCorrector` entry points those cops use, shared by
//! `Layout/BeginEndAlignment`, `Layout/DefEndAlignment`,
//! `Layout/EndAlignment`, `Layout/ElseAlignment` and
//! `Layout/AssignmentIndentation`.
//!
//! Upstream builds a `{style => range}` hash for every supported style and
//! then looks only at the configured one (`matching.key?(style)` plus
//! `align_ranges[style]` for the message; the other keys feed
//! `style_detected`, which only drives `--auto-gen-config`). This port
//! therefore computes just the one range the configured style asks for.

use linter::{Context, Edit, OptionValue, RuleOptions};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// RuboCop's `EndKeywordAlignment::MSG` (`Layout/BeginEndAlignment` and
/// `Layout/DefEndAlignment` restate the same text as their own `MSG`).
pub(crate) fn misalignment_message(ctx: &Context<'_>, end_loc: Span, align_with: Span) -> String {
    let end_pos = ctx.line_col(end_loc.start);
    let align_pos = ctx.line_col(align_with.start);
    let source = String::from_utf8_lossy(ctx.text(align_with));
    format!(
        "`end` at {}, {} is not aligned with `{source}` at {}, {}.",
        end_pos.line, end_pos.column, align_pos.line, align_pos.column
    )
}

/// RuboCop's `RangeHelp#effective_column`: the character column, less the
/// byte-order mark on line 1 of a file that starts with one.
pub(crate) fn effective_column(ctx: &Context<'_>, span: Span) -> i64 {
    let line_col = ctx.line_col(span.start);
    let mut column = i64::from(line_col.column);
    if line_col.line == 1 && ctx.source().bytes().starts_with(&[0xEF, 0xBB, 0xBF]) {
        column -= 1;
    }
    column
}

/// RuboCop's `RangeHelp#column_offset_between`.
pub(crate) fn column_offset_between(ctx: &Context<'_>, base: Span, range: Span) -> i64 {
    effective_column(ctx, base) - effective_column(ctx, range)
}

/// RuboCop's `EndKeywordAlignment#matching_ranges`, narrowed to the one
/// configured style: the `end` keyword shares `align_with`'s line, or sits
/// in the same column.
pub(crate) fn end_is_aligned(ctx: &Context<'_>, align_with: Span, end_loc: Span) -> bool {
    ctx.same_line(align_with, end_loc) || column_offset_between(ctx, align_with, end_loc) == 0
}

/// RuboCop's `EndKeywordAlignment#start_line_range`: the stretch of the
/// line `span` starts on that runs from its first non-blank character
/// through its last one.
pub(crate) fn start_line_range(ctx: &Context<'_>, span: Span) -> Span {
    let line = ctx.line_col(span.start).line;
    let line_span = ctx.line_span(line);
    let text = ctx.line_text(line);
    let first = text.iter().position(|b| !is_blank(*b)).unwrap_or(0);
    let last = text.iter().rposition(|b| !is_blank(*b)).map_or(first, |i| i + 1);
    let start = line_span.start + u32::try_from(first).unwrap_or(0);
    let end = line_span.start + u32::try_from(last).unwrap_or(0);
    Span::new(start, end.max(start))
}

/// Ruby's `\s` inside `String#=~`, which `start_line_range` matches with.
fn is_blank(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | 0x0B | 0x0C)
}

/// `Layout/IndentationStyle`'s `EnforcedStyle`, which
/// `AlignmentCorrector.using_tabs?` consults.
pub(crate) fn uses_tabs(options: &RuleOptions) -> bool {
    options.peer("Layout/IndentationStyle", "EnforcedStyle").and_then(OptionValue::as_str)
        == Some("tabs")
}

/// RuboCop's `AlignmentCorrector.align_end`: re-indent the `end` keyword to
/// `column`, or break its line first when code precedes it.
pub(crate) fn align_end(ctx: &Context<'_>, end_loc: Span, column: u32, tabs: bool) -> Option<Edit> {
    let line = ctx.line_col(end_loc.start).line;
    let line_start = ctx.line_span(line).start;
    let whitespace = Span::new(line_start, end_loc.start);
    let unit = if tabs { b'\t' } else { b' ' };
    let indentation = vec![unit; usize::try_from(column).unwrap_or(0)];
    if ctx.text(whitespace).iter().all(|b| is_blank(*b) || *b == 0) {
        if ctx.text(whitespace) == indentation.as_slice() {
            return None;
        }
        Some(Edit::replace(whitespace, indentation))
    } else {
        let mut insert = Vec::with_capacity(indentation.len() + 1);
        insert.push(b'\n');
        insert.extend_from_slice(&indentation);
        Some(Edit::insert(whitespace.end, insert))
    }
}

/// RuboCop's `Util#first_part_of_call_chain`, calling `step` for every node
/// walked past so a caller can rebuild the receiver chain as the ancestor
/// list of the node this returns.
pub(crate) fn first_part_of_call_chain<'pr>(
    node: Node<'pr>,
    mut step: impl FnMut(&Node<'pr>),
) -> Option<Node<'pr>> {
    let mut current = Some(node);
    while let Some(node) = current {
        // Prism has no separate `block` node: a call with a literal block is
        // one `CallNode`, so whitequark's `any_block_type? -> send_node` hop
        // is already folded into the `call_type? -> receiver` hop.
        let Node::CallNode { .. } = node else { return Some(node) };
        let call = node.as_call_node().expect("kind matched");
        step(&node);
        current = call.receiver();
    }
    None
}

/// RuboCop's `CheckAssignment.extract_rhs`.
///
/// Whitequark counts a block-pass argument (`&blk`) as an ordinary
/// argument, so `last_argument` is the `block_pass` node for a call that
/// has one; Prism keeps it out of `arguments`. Returning the
/// `BlockArgumentNode` restores upstream's answer.
pub(crate) fn extract_rhs<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    match node {
        Node::CallNode { .. } => {
            let call = node.as_call_node().expect("kind matched");
            last_argument(&call)
        }
        Node::LocalVariableWriteNode { .. } => {
            Some(node.as_local_variable_write_node().expect("kind matched").value())
        }
        Node::InstanceVariableWriteNode { .. } => {
            Some(node.as_instance_variable_write_node().expect("kind matched").value())
        }
        Node::ClassVariableWriteNode { .. } => {
            Some(node.as_class_variable_write_node().expect("kind matched").value())
        }
        Node::GlobalVariableWriteNode { .. } => {
            Some(node.as_global_variable_write_node().expect("kind matched").value())
        }
        Node::ConstantWriteNode { .. } => {
            Some(node.as_constant_write_node().expect("kind matched").value())
        }
        Node::ConstantPathWriteNode { .. } => {
            Some(node.as_constant_path_write_node().expect("kind matched").value())
        }
        Node::MultiWriteNode { .. } => {
            Some(node.as_multi_write_node().expect("kind matched").value())
        }
        Node::LocalVariableAndWriteNode { .. } => {
            Some(node.as_local_variable_and_write_node().expect("kind matched").value())
        }
        Node::LocalVariableOrWriteNode { .. } => {
            Some(node.as_local_variable_or_write_node().expect("kind matched").value())
        }
        Node::LocalVariableOperatorWriteNode { .. } => {
            Some(node.as_local_variable_operator_write_node().expect("kind matched").value())
        }
        Node::InstanceVariableAndWriteNode { .. } => {
            Some(node.as_instance_variable_and_write_node().expect("kind matched").value())
        }
        Node::InstanceVariableOrWriteNode { .. } => {
            Some(node.as_instance_variable_or_write_node().expect("kind matched").value())
        }
        Node::InstanceVariableOperatorWriteNode { .. } => {
            Some(node.as_instance_variable_operator_write_node().expect("kind matched").value())
        }
        Node::ClassVariableAndWriteNode { .. } => {
            Some(node.as_class_variable_and_write_node().expect("kind matched").value())
        }
        Node::ClassVariableOrWriteNode { .. } => {
            Some(node.as_class_variable_or_write_node().expect("kind matched").value())
        }
        Node::ClassVariableOperatorWriteNode { .. } => {
            Some(node.as_class_variable_operator_write_node().expect("kind matched").value())
        }
        Node::GlobalVariableAndWriteNode { .. } => {
            Some(node.as_global_variable_and_write_node().expect("kind matched").value())
        }
        Node::GlobalVariableOrWriteNode { .. } => {
            Some(node.as_global_variable_or_write_node().expect("kind matched").value())
        }
        Node::GlobalVariableOperatorWriteNode { .. } => {
            Some(node.as_global_variable_operator_write_node().expect("kind matched").value())
        }
        Node::ConstantAndWriteNode { .. } => {
            Some(node.as_constant_and_write_node().expect("kind matched").value())
        }
        Node::ConstantOrWriteNode { .. } => {
            Some(node.as_constant_or_write_node().expect("kind matched").value())
        }
        Node::ConstantOperatorWriteNode { .. } => {
            Some(node.as_constant_operator_write_node().expect("kind matched").value())
        }
        Node::ConstantPathAndWriteNode { .. } => {
            Some(node.as_constant_path_and_write_node().expect("kind matched").value())
        }
        Node::ConstantPathOrWriteNode { .. } => {
            Some(node.as_constant_path_or_write_node().expect("kind matched").value())
        }
        Node::ConstantPathOperatorWriteNode { .. } => {
            Some(node.as_constant_path_operator_write_node().expect("kind matched").value())
        }
        Node::CallAndWriteNode { .. } => {
            Some(node.as_call_and_write_node().expect("kind matched").value())
        }
        Node::CallOrWriteNode { .. } => {
            Some(node.as_call_or_write_node().expect("kind matched").value())
        }
        Node::CallOperatorWriteNode { .. } => {
            Some(node.as_call_operator_write_node().expect("kind matched").value())
        }
        Node::IndexAndWriteNode { .. } => {
            Some(node.as_index_and_write_node().expect("kind matched").value())
        }
        Node::IndexOrWriteNode { .. } => {
            Some(node.as_index_or_write_node().expect("kind matched").value())
        }
        Node::IndexOperatorWriteNode { .. } => {
            Some(node.as_index_operator_write_node().expect("kind matched").value())
        }
        _ => None,
    }
}

/// `rubocop-ast`'s `MethodDispatchNode#last_argument`, with whitequark's
/// block-pass argument restored.
fn last_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    if let Some(block) = call.block() {
        if let Node::BlockArgumentNode { .. } = block {
            return Some(block);
        }
    }
    call.arguments()?.arguments().iter().last()
}

/// The `=`/`+=`/`||=` token of an assignment node -- whitequark's
/// `node.loc.operator`, which a plain (non-assigning) call does not have.
pub(crate) fn assignment_operator(node: &Node<'_>) -> Option<Span> {
    simple_write_operator(node).or_else(|| compound_write_operator(node)).map(|l| l.span())
}

/// `assignment_operator`'s plain-write arms: `send` and the non-compound
/// whitequark write node kinds.
fn simple_write_operator<'pr>(node: &Node<'pr>) -> Option<ruby_ast::Location<'pr>> {
    match node {
        Node::CallNode { .. } => node.as_call_node().expect("kind matched").equal_loc(),
        Node::LocalVariableWriteNode { .. } => {
            Some(node.as_local_variable_write_node().expect("kind matched").operator_loc())
        }
        Node::InstanceVariableWriteNode { .. } => {
            Some(node.as_instance_variable_write_node().expect("kind matched").operator_loc())
        }
        Node::ClassVariableWriteNode { .. } => {
            Some(node.as_class_variable_write_node().expect("kind matched").operator_loc())
        }
        Node::GlobalVariableWriteNode { .. } => {
            Some(node.as_global_variable_write_node().expect("kind matched").operator_loc())
        }
        Node::ConstantWriteNode { .. } => {
            Some(node.as_constant_write_node().expect("kind matched").operator_loc())
        }
        Node::ConstantPathWriteNode { .. } => {
            Some(node.as_constant_path_write_node().expect("kind matched").operator_loc())
        }
        Node::MultiWriteNode { .. } => {
            Some(node.as_multi_write_node().expect("kind matched").operator_loc())
        }
        _ => None,
    }
}

/// `assignment_operator`'s compound-write arms: the `&&=`/`||=`/`op=`
/// whitequark write node kinds.
fn compound_write_operator<'pr>(node: &Node<'pr>) -> Option<ruby_ast::Location<'pr>> {
    match node {
        Node::LocalVariableAndWriteNode { .. } => {
            Some(node.as_local_variable_and_write_node().expect("kind matched").operator_loc())
        }
        Node::LocalVariableOrWriteNode { .. } => {
            Some(node.as_local_variable_or_write_node().expect("kind matched").operator_loc())
        }
        Node::LocalVariableOperatorWriteNode { .. } => Some(
            node.as_local_variable_operator_write_node()
                .expect("kind matched")
                .binary_operator_loc(),
        ),
        Node::InstanceVariableAndWriteNode { .. } => {
            Some(node.as_instance_variable_and_write_node().expect("kind matched").operator_loc())
        }
        Node::InstanceVariableOrWriteNode { .. } => {
            Some(node.as_instance_variable_or_write_node().expect("kind matched").operator_loc())
        }
        Node::InstanceVariableOperatorWriteNode { .. } => Some(
            node.as_instance_variable_operator_write_node()
                .expect("kind matched")
                .binary_operator_loc(),
        ),
        Node::ClassVariableAndWriteNode { .. } => {
            Some(node.as_class_variable_and_write_node().expect("kind matched").operator_loc())
        }
        Node::ClassVariableOrWriteNode { .. } => {
            Some(node.as_class_variable_or_write_node().expect("kind matched").operator_loc())
        }
        Node::ClassVariableOperatorWriteNode { .. } => Some(
            node.as_class_variable_operator_write_node()
                .expect("kind matched")
                .binary_operator_loc(),
        ),
        Node::GlobalVariableAndWriteNode { .. } => {
            Some(node.as_global_variable_and_write_node().expect("kind matched").operator_loc())
        }
        Node::GlobalVariableOrWriteNode { .. } => {
            Some(node.as_global_variable_or_write_node().expect("kind matched").operator_loc())
        }
        Node::GlobalVariableOperatorWriteNode { .. } => Some(
            node.as_global_variable_operator_write_node()
                .expect("kind matched")
                .binary_operator_loc(),
        ),
        Node::ConstantAndWriteNode { .. } => {
            Some(node.as_constant_and_write_node().expect("kind matched").operator_loc())
        }
        Node::ConstantOrWriteNode { .. } => {
            Some(node.as_constant_or_write_node().expect("kind matched").operator_loc())
        }
        Node::ConstantOperatorWriteNode { .. } => Some(
            node.as_constant_operator_write_node().expect("kind matched").binary_operator_loc(),
        ),
        Node::ConstantPathAndWriteNode { .. } => {
            Some(node.as_constant_path_and_write_node().expect("kind matched").operator_loc())
        }
        Node::ConstantPathOrWriteNode { .. } => {
            Some(node.as_constant_path_or_write_node().expect("kind matched").operator_loc())
        }
        Node::ConstantPathOperatorWriteNode { .. } => Some(
            node.as_constant_path_operator_write_node()
                .expect("kind matched")
                .binary_operator_loc(),
        ),
        Node::CallAndWriteNode { .. } => {
            Some(node.as_call_and_write_node().expect("kind matched").operator_loc())
        }
        Node::CallOrWriteNode { .. } => {
            Some(node.as_call_or_write_node().expect("kind matched").operator_loc())
        }
        Node::CallOperatorWriteNode { .. } => {
            Some(node.as_call_operator_write_node().expect("kind matched").binary_operator_loc())
        }
        Node::IndexAndWriteNode { .. } => {
            Some(node.as_index_and_write_node().expect("kind matched").operator_loc())
        }
        Node::IndexOrWriteNode { .. } => {
            Some(node.as_index_or_write_node().expect("kind matched").operator_loc())
        }
        Node::IndexOperatorWriteNode { .. } => {
            Some(node.as_index_operator_write_node().expect("kind matched").binary_operator_loc())
        }
        _ => None,
    }
}

/// The node kinds `CheckAssignment` subscribes to: every whitequark
/// assignment type plus `send`.
pub(crate) const ASSIGNMENT_KINDS: &[NodeKind] = &[
    NodeKind::CallNode,
    NodeKind::LocalVariableWriteNode,
    NodeKind::InstanceVariableWriteNode,
    NodeKind::ClassVariableWriteNode,
    NodeKind::GlobalVariableWriteNode,
    NodeKind::ConstantWriteNode,
    NodeKind::ConstantPathWriteNode,
    NodeKind::MultiWriteNode,
    NodeKind::LocalVariableAndWriteNode,
    NodeKind::LocalVariableOrWriteNode,
    NodeKind::LocalVariableOperatorWriteNode,
    NodeKind::InstanceVariableAndWriteNode,
    NodeKind::InstanceVariableOrWriteNode,
    NodeKind::InstanceVariableOperatorWriteNode,
    NodeKind::ClassVariableAndWriteNode,
    NodeKind::ClassVariableOrWriteNode,
    NodeKind::ClassVariableOperatorWriteNode,
    NodeKind::GlobalVariableAndWriteNode,
    NodeKind::GlobalVariableOrWriteNode,
    NodeKind::GlobalVariableOperatorWriteNode,
    NodeKind::ConstantAndWriteNode,
    NodeKind::ConstantOrWriteNode,
    NodeKind::ConstantOperatorWriteNode,
    NodeKind::ConstantPathAndWriteNode,
    NodeKind::ConstantPathOrWriteNode,
    NodeKind::ConstantPathOperatorWriteNode,
    NodeKind::CallAndWriteNode,
    NodeKind::CallOrWriteNode,
    NodeKind::CallOperatorWriteNode,
    NodeKind::IndexAndWriteNode,
    NodeKind::IndexOrWriteNode,
    NodeKind::IndexOperatorWriteNode,
];

/// `rubocop-ast`'s `Node#assignment?` (`EQUALS_ASSIGNMENTS` plus
/// `SHORTHAND_ASSIGNMENTS`), for everything but a call.
///
/// `SendNode` overrides the predicate with `setter_method?`
/// (`loc?(:operator)`), so a setter call such as `foo.bar = 1` or
/// `h[k] = 1` counts as an assignment too; that needs the node itself, so
/// callers test [`is_setter_call`] separately.
pub(crate) fn is_assignment_kind(kind: NodeKind) -> bool {
    kind != NodeKind::CallNode && ASSIGNMENT_KINDS.contains(&kind)
}

/// `rubocop-ast`'s `MethodDispatchNode#setter_method?`, which `SendNode`
/// aliases as `assignment?`.
pub(crate) fn is_setter_call(call: &CallNode<'_>) -> bool {
    call.equal_loc().is_some()
}

/// RuboCop's `node.parent` for a node Prism nests inside an `ArgumentsNode`
/// wrapper: whitequark has no such node, so the call itself is the parent.
pub(crate) fn logical_parent(ctx: &Context<'_>) -> Option<linter::NodeInfo> {
    let ancestors = ctx.ancestors();
    let parent = *ancestors.last()?;
    if parent.kind == NodeKind::ArgumentsNode {
        ancestors.get(ancestors.len().checked_sub(2)?).copied()
    } else {
        Some(parent)
    }
}
