//! `Style/MapJoin`, ported from RuboCop's `lib/rubocop/cop/style/map_join.rb`.
//!
//! whitequark's `block` node wraps its call (`(block (send recv :map) args
//! body)`); Prism inverts this -- the `map`/`collect` `CallNode` owns the
//! block through its own `block` field (`Some(Node::BlockNode)`), so
//! upstream's separate `map_node`/`map_send` (the block vs. the call it
//! wraps) collapse into the one `CallNode` here. A `&:to_s` block-pass is
//! `CallNode::block` as `Some(Node::BlockArgumentNode)` (whitequark's
//! `block_pass` argument-list element); a literal block, braces or
//! `do`/`end`, is `Some(Node::BlockNode)` regardless of whether its
//! parameter is written `|x|`, `_1`/`_2` (`NumberedParametersNode`,
//! restricted to `maximum == 1`), or `it` (`ItParametersNode`, whose body
//! reads back as `ItLocalVariableReadNode` rather than
//! `LocalVariableReadNode`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks for redundant `map(&:to_s)` before `join`.
#[derive(Debug, Clone)]
pub struct MapJoin;

impl Rule for MapJoin {
    const META: RuleMeta = RuleMeta {
        name: "Style/MapJoin",
        department: Department::Style,
        summary: "Checks for redundant `map(&:to_s)` before `join`.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(join) = node.as_call_node() else { return };
        if join.name().as_slice() != b"join" {
            return;
        }
        let Some(receiver) = join.receiver() else { return };
        let Some(map_call) = receiver.as_call_node() else { return };
        let method = map_call.name();
        let method = method.as_slice();
        if method != b"map" && method != b"collect" {
            return;
        }
        if !maps_to_s(&map_call) {
            return;
        }
        let Some(selector) = map_call.message_loc() else { return };
        let method_name = String::from_utf8_lossy(method);
        let message = format!("Remove redundant `{method_name}(&:to_s)` before `join`.");

        let removal = if let Some(map_receiver) = map_call.receiver() {
            let dot = map_call.call_operator_loc().expect("has a block, so has a dot");
            let start = if ctx.last_line(map_receiver.span()) < ctx.line_col(dot.span().start).line
            {
                map_receiver.span().end
            } else {
                dot.span().start
            };
            Span::new(start, map_call.as_node().span().end)
        } else {
            let join_dot = join.call_operator_loc().expect("has a receiver, so has a dot");
            Span::new(map_call.as_node().span().start, join_dot.span().end)
        };

        ctx.report_with_fix(
            &Self::META,
            selector.span(),
            message,
            Fix { applicability: Applicability::Unsafe, edits: vec![Edit::delete(removal)] },
        );
    }
}

/// `map_to_s_join?`/`map_to_s_block_join?`/`map_to_s_numblock_join?`/
/// `map_to_s_itblock_join?` collapsed into one check over `map_call`'s own
/// `block` field.
fn maps_to_s(map_call: &CallNode<'_>) -> bool {
    match map_call.block() {
        Some(Node::BlockArgumentNode { .. }) => {
            let Some(block_arg) = map_call.block().and_then(|b| b.as_block_argument_node()) else {
                return false;
            };
            block_arg
                .expression()
                .and_then(|e| e.as_symbol_node())
                .is_some_and(|sym| sym.unescaped() == b"to_s")
        }
        Some(Node::BlockNode { .. }) => {
            let Some(block) = map_call.block().and_then(|b| b.as_block_node()) else {
                return false;
            };
            block_calls_to_s(&block)
        }
        _ => false,
    }
}

/// Whether a literal block's sole statement is `<param>.to_s` with no
/// arguments, for every parameter shape Prism represents.
fn block_calls_to_s(block: &ruby_ast::node::BlockNode<'_>) -> bool {
    let Some(statements) = block.body().and_then(|b| b.as_statements_node()) else { return false };
    if statements.body().len() != 1 {
        return false;
    }
    let Some(call) = statements.body().first().and_then(|n| n.as_call_node()) else { return false };
    if call.name().as_slice() != b"to_s" || call.arguments().is_some() || call.block().is_some() {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };

    match block.parameters() {
        Some(Node::BlockParametersNode { .. }) => {
            let Some(params) = block.parameters().and_then(|p| p.as_block_parameters_node()) else {
                return false;
            };
            let Some(required) = params
                .parameters()
                .and_then(|p| p.requireds().first())
                .and_then(|a| a.as_required_parameter_node())
            else {
                return false;
            };
            receiver
                .as_local_variable_read_node()
                .is_some_and(|lvar| lvar.name().as_slice() == required.name().as_slice())
        }
        Some(Node::NumberedParametersNode { .. }) => {
            let Some(numbered) = block.parameters().and_then(|p| p.as_numbered_parameters_node())
            else {
                return false;
            };
            numbered.maximum() == 1
                && receiver
                    .as_local_variable_read_node()
                    .is_some_and(|lvar| lvar.name().as_slice() == b"_1")
        }
        Some(Node::ItParametersNode { .. }) => receiver.as_it_local_variable_read_node().is_some(),
        _ => false,
    }
}
