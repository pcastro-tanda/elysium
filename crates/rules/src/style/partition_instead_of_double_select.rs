//! `Style/PartitionInsteadOfDoubleSelect`, ported from RuboCop's
//! `lib/rubocop/cop/style/partition_instead_of_double_select.rb`.
//!
//! Upstream reaches its two call sites (the later of a pair of statements)
//! via `on_block`/`on_send`, then walks *up* to find the enclosing statement
//! (`node_container`) and *sideways* to its `left_sibling` to find the
//! earlier one. Prism's generic traversal only hands a rule a node's
//! ancestors' kind and span (not a live reference it could use to look up
//! siblings), so this port instead subscribes to
//! [`NodeKind::StatementsNode`] and walks each one's own `body()` list in
//! adjacent pairs directly -- the same restructuring
//! `style/combinable_loops.rs` uses for an identical "look at the previous
//! statement" problem. This also makes upstream's `node_container`
//! (`parent.begin_type?` / `parent.assignment? && parent.parent.begin_type?`)
//! redundant: every element of a `StatementsNode`'s `body()` already *is* a
//! top-level statement, so [`extract_candidate`] only has to look at the
//! statement itself (or, if it is an assignment, its value) without any
//! upward walk.
//!
//! whitequark's `node.last_argument&.block_pass_type?` (a block-pass
//! *argument* on the end of the regular argument list) is, in Prism, the
//! call's own `block()` field holding a [`NodeKind::BlockArgumentNode`]
//! instead of a [`NodeKind::BlockNode`] -- the same `&blk`-vs-literal-block
//! split noted throughout this codebase (see `KIT.md`). Since Prism's
//! `CallNode` span already includes its attached block (literal or passed),
//! `build_partition_call`'s whitequark-specific `node.any_block_type? ?
//! node.send_node : node` split collapses to always reading the call's own
//! `message_loc`/span, regardless of which shape the block took.
//!
//! `Node#==` (`same_block_contents?`'s `block1.arguments == block2.arguments`
//! / `block1.body == block2.body`, `node1.last_argument ==
//! node2.last_argument`, `negated_body?`'s receiver comparison) is
//! approximated by source-text equality throughout, the same convention
//! `combinable_loops.rs` and `redundant_safe_navigation.rs` use for the same
//! problem.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, CallNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const SELECT_METHODS: &[&[u8]] = &[b"select", b"filter", b"find_all"];

/// A statement's `select`/`filter`/`find_all`/`reject` call, found either
/// directly or as the value of an assignment. `block` is `None` for a
/// block-pass call (`arr.select(&:positive?)`).
struct Candidate<'pr> {
    call: CallNode<'pr>,
    block: Option<BlockNode<'pr>>,
}

/// Checks for consecutive `select`/`filter`/`find_all` and `reject` calls
/// on the same receiver with the same block body.
#[derive(Debug, Clone)]
pub struct PartitionInsteadOfDoubleSelect;

impl Rule for PartitionInsteadOfDoubleSelect {
    const META: RuleMeta = RuleMeta {
        name: "Style/PartitionInsteadOfDoubleSelect",
        department: Department::Style,
        summary: "Checks for consecutive `select`/`filter`/`find_all` and `reject` calls on the \
            same receiver with the same block body.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let stmts = node.as_statements_node().expect("kind matched");
        let body: Vec<Node<'_>> = stmts.body().iter().collect();
        for pair in body.windows(2) {
            check_pair(ctx, &pair[0], &pair[1]);
        }
    }
}

fn is_select(name: &[u8]) -> bool {
    SELECT_METHODS.contains(&name)
}

fn is_candidate_method(name: &[u8]) -> bool {
    is_select(name) || name == b"reject"
}

/// Generic `Node#assignment?`'s value: the RHS of a plain `lvasgn`/`ivasgn`/
/// `cvasgn`/`gvasgn`/`casgn`, or `None` if `node` is not one of those.
fn assignment_value<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => Some(node.as_local_variable_write_node()?.value()),
        NodeKind::InstanceVariableWriteNode => {
            Some(node.as_instance_variable_write_node()?.value())
        }
        NodeKind::ClassVariableWriteNode => Some(node.as_class_variable_write_node()?.value()),
        NodeKind::GlobalVariableWriteNode => Some(node.as_global_variable_write_node()?.value()),
        NodeKind::ConstantWriteNode => Some(node.as_constant_write_node()?.value()),
        _ => None,
    }
}

/// The source span of an assignment's own LHS name (including any sigil),
/// used to build the `a, b = ...` replacement's variable names.
fn assignment_name_span(node: &Node<'_>) -> Option<Span> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => {
            Some(node.as_local_variable_write_node()?.name_loc().span())
        }
        NodeKind::InstanceVariableWriteNode => {
            Some(node.as_instance_variable_write_node()?.name_loc().span())
        }
        NodeKind::ClassVariableWriteNode => {
            Some(node.as_class_variable_write_node()?.name_loc().span())
        }
        NodeKind::GlobalVariableWriteNode => {
            Some(node.as_global_variable_write_node()?.name_loc().span())
        }
        NodeKind::ConstantWriteNode => Some(node.as_constant_write_node()?.name_loc().span()),
        _ => None,
    }
}

/// RuboCop's `extract_candidate` (`extract_block` + `extract_block_pass_send`),
/// applied directly to a top-level statement rather than to a container
/// upstream had to first walk up to find (see the module docs).
fn extract_candidate<'pr>(stmt: &Node<'pr>) -> Option<Candidate<'pr>> {
    let node = assignment_value(stmt).unwrap_or(*stmt);
    let call = node.as_call_node()?;
    if !is_candidate_method(call.name().as_slice()) {
        return None;
    }
    let block = call.block()?;
    if let Some(b) = block.as_block_node() {
        return Some(Candidate { call, block: Some(b) });
    }
    block.as_block_argument_node()?;
    Some(Candidate { call, block: None })
}

/// RuboCop's `node.receiver == sibling.receiver`.
fn same_optional_node(ctx: &Context<'_>, a: Option<Node<'_>>, b: Option<Node<'_>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => ctx.text(a.span()) == ctx.text(b.span()),
        _ => false,
    }
}

/// RuboCop's `symbol_proc_method?`: `(block _ (args (arg _name)) (send
/// (lvar _name) $_method_name))` -- a block with exactly one plain required
/// parameter whose single-statement body is a bare, argument-less call on
/// that same parameter.
fn symbol_proc_method_name(block: &BlockNode<'_>) -> Option<Vec<u8>> {
    let params = block.parameters()?;
    let block_params = params.as_block_parameters_node()?;
    let parameters = block_params.parameters()?;
    if !parameters.optionals().is_empty()
        || parameters.rest().is_some()
        || !parameters.posts().is_empty()
        || !parameters.keywords().is_empty()
        || parameters.keyword_rest().is_some()
        || parameters.block().is_some()
    {
        return None;
    }
    let requireds: Vec<Node<'_>> = parameters.requireds().iter().collect();
    let [only] = requireds.as_slice() else { return None };
    let arg_name = only.as_required_parameter_node()?.name().as_slice().to_vec();

    let body = single_statement(block)?;
    let call = body.as_call_node()?;
    if call.is_safe_navigation() || call.arguments().is_some() {
        return None;
    }
    let receiver = call.receiver()?;
    let lvar = receiver.as_local_variable_read_node()?;
    if lvar.name().as_slice() != arg_name {
        return None;
    }
    Some(call.name().as_slice().to_vec())
}

/// The block body's sole statement, or `None` for an empty or
/// multi-statement body.
fn single_statement<'pr>(block: &BlockNode<'pr>) -> Option<Node<'pr>> {
    let stmts = block.body()?.as_statements_node()?;
    let items: Vec<Node<'_>> = stmts.body().iter().collect();
    let [stmt] = items.as_slice() else { return None };
    Some(*stmt)
}

/// `&expr`'s passed expression, for a block-pass candidate.
fn block_pass_expr<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    call.block()?.as_block_argument_node()?.expression()
}

/// RuboCop's `block_matches_block_pass?`.
fn block_matches_block_pass(block: &BlockNode<'_>, send: &CallNode<'_>) -> bool {
    let Some(method_name) = symbol_proc_method_name(block) else { return false };
    let Some(sym) = block_pass_expr(send).and_then(|e| e.as_symbol_node()) else { return false };
    sym.unescaped() == method_name
}

/// A block's "type" for upstream's `block1.type == block2.type`: the
/// `:block`/`:numblock`/`:itblock` distinction, derived from its parameter
/// shape.
#[derive(PartialEq, Eq)]
enum BlockShape {
    Regular,
    Numbered,
    It,
}

fn block_shape(block: &BlockNode<'_>) -> BlockShape {
    match block.parameters().map(|p| p.kind()) {
        Some(NodeKind::NumberedParametersNode) => BlockShape::Numbered,
        Some(NodeKind::ItParametersNode) => BlockShape::It,
        _ => BlockShape::Regular,
    }
}

/// RuboCop's `same_block_contents?`.
fn same_block_contents(ctx: &Context<'_>, a: &BlockNode<'_>, b: &BlockNode<'_>) -> bool {
    if block_shape(a) != block_shape(b) {
        return false;
    }
    if block_shape(a) == BlockShape::Regular {
        let params_a = a.parameters().map(|p| p.span());
        let params_b = b.parameters().map(|p| p.span());
        if !same_optional_span(ctx, params_a, params_b) {
            return false;
        }
    }
    same_optional_span(ctx, a.body().map(|n| n.span()), b.body().map(|n| n.span()))
}

fn same_optional_span(ctx: &Context<'_>, a: Option<Span>, b: Option<Span>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => ctx.text(a) == ctx.text(b),
        _ => false,
    }
}

/// RuboCop's `equivalent_predicate?`.
fn equivalent_predicate(ctx: &Context<'_>, n1: &Candidate<'_>, n2: &Candidate<'_>) -> bool {
    match (&n1.block, &n2.block) {
        (Some(b1), Some(b2)) => same_block_contents(ctx, b1, b2),
        (Some(b1), None) => block_matches_block_pass(b1, &n2.call),
        (None, Some(b2)) => block_matches_block_pass(b2, &n1.call),
        (None, None) => {
            same_optional_node(ctx, block_pass_expr(&n1.call), block_pass_expr(&n2.call))
        }
    }
}

/// RuboCop's `negated_body?`.
fn negated_body(ctx: &Context<'_>, a: &BlockNode<'_>, b: &BlockNode<'_>) -> bool {
    let Some(body_a) = single_statement(a) else { return false };
    let Some(body_b) = single_statement(b) else { return false };
    let Some(call) = body_a.as_call_node() else { return false };
    if call.name().as_slice() != b"!" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    ctx.text(receiver.span()) == ctx.text(body_b.span())
}

/// RuboCop's `negated_predicate?`.
fn negated_predicate(ctx: &Context<'_>, n1: &Candidate<'_>, n2: &Candidate<'_>) -> bool {
    let (Some(b1), Some(b2)) = (&n1.block, &n2.block) else { return false };
    if block_shape(b1) != block_shape(b2) {
        return false;
    }
    if block_shape(b1) == BlockShape::Regular {
        let params_a = b1.parameters().map(|p| p.span());
        let params_b = b2.parameters().map(|p| p.span());
        if !same_optional_span(ctx, params_a, params_b) {
            return false;
        }
    }
    negated_body(ctx, b1, b2) || negated_body(ctx, b2, b1)
}

/// RuboCop's `matching_pair?`.
fn matching_pair(ctx: &Context<'_>, node: &Candidate<'_>, sibling: &Candidate<'_>) -> bool {
    let complementary = (is_select(node.call.name().as_slice())
        && sibling.call.name().as_slice() == b"reject")
        || (node.call.name().as_slice() == b"reject" && is_select(sibling.call.name().as_slice()));
    (complementary && equivalent_predicate(ctx, node, sibling))
        || (node.call.name().as_slice() == sibling.call.name().as_slice()
            && negated_predicate(ctx, node, sibling))
}

/// RuboCop's `build_partition_call`: Prism's `CallNode` span already
/// includes any attached block (literal or passed), so unlike upstream
/// there is no `any_block_type? ? node.send_node : node` split -- the
/// call's own span and `message_loc` are always what is needed.
fn build_partition_call(ctx: &Context<'_>, call: &CallNode<'_>) -> String {
    let full = call.as_node().span();
    let selector = call.message_loc().expect("call always has a selector").span();
    let source = ctx.text(full);
    let method_start = (selector.start - full.start) as usize;
    let method_end = (selector.end - full.start) as usize;
    format!(
        "{}partition{}",
        String::from_utf8_lossy(&source[..method_start]),
        String::from_utf8_lossy(&source[method_end..])
    )
}

/// RuboCop's `complementary_variable_order` + `select_node_for`.
fn complementary_variables<'pr>(
    sibling: &Candidate<'pr>,
    sibling_stmt: &Node<'pr>,
    container: &Candidate<'pr>,
    container_stmt: &Node<'pr>,
) -> (Span, Span, CallNode<'pr>) {
    let (select_var, reject_var) = if is_select(sibling.call.name().as_slice()) {
        (
            assignment_name_span(sibling_stmt).expect("both_lvasgn? checked"),
            assignment_name_span(container_stmt).expect("both_lvasgn? checked"),
        )
    } else {
        (
            assignment_name_span(container_stmt).expect("both_lvasgn? checked"),
            assignment_name_span(sibling_stmt).expect("both_lvasgn? checked"),
        )
    };
    let partition_call =
        if is_select(sibling.call.name().as_slice()) { sibling.call } else { container.call };
    (select_var, reject_var, partition_call)
}

/// RuboCop's `negation_partition_args`.
fn negation_partition_vars<'pr>(
    ctx: &Context<'_>,
    node: &Candidate<'pr>,
    node_stmt: &Node<'pr>,
    sibling: &Candidate<'pr>,
    sibling_stmt: &Node<'pr>,
) -> (Span, Span, CallNode<'pr>) {
    let (Some(node_block), Some(sibling_block)) = (&node.block, &sibling.block) else {
        unreachable!("negated_predicate? requires both to be real blocks")
    };
    let node_is_negated = negated_body(ctx, node_block, sibling_block);
    let is_select = is_select(node.call.name().as_slice());
    let node_is_truthy = is_select != node_is_negated;
    let partition_call = if node_is_negated { sibling.call } else { node.call };

    let (a, b) = if node_is_truthy {
        (
            assignment_name_span(node_stmt).expect("both_lvasgn? checked"),
            assignment_name_span(sibling_stmt).expect("both_lvasgn? checked"),
        )
    } else {
        (
            assignment_name_span(sibling_stmt).expect("both_lvasgn? checked"),
            assignment_name_span(node_stmt).expect("both_lvasgn? checked"),
        )
    };
    (a, b, partition_call)
}

/// RuboCop's `on_block`/`on_numblock`/`on_itblock`/`on_send`'s shared
/// `find_and_register_offense`, applied to one adjacent statement pair.
fn check_pair(ctx: &mut Context<'_>, prev_stmt: &Node<'_>, cur_stmt: &Node<'_>) {
    let Some(prev) = extract_candidate(prev_stmt) else { return };
    let Some(cur) = extract_candidate(cur_stmt) else { return };
    if !same_optional_node(ctx, prev.call.receiver(), cur.call.receiver()) {
        return;
    }
    if !matching_pair(ctx, &cur, &prev) {
        return;
    }

    let first = String::from_utf8_lossy(prev.call.name().as_slice()).into_owned();
    let second = String::from_utf8_lossy(cur.call.name().as_slice()).into_owned();
    let message = format!("Use `partition` instead of consecutive `{first}` and `{second}` calls.");
    let span = cur_stmt.span();

    let both_lvasgn = prev_stmt.kind() == NodeKind::LocalVariableWriteNode
        && cur_stmt.kind() == NodeKind::LocalVariableWriteNode;
    if !both_lvasgn {
        ctx.report(&PartitionInsteadOfDoubleSelect::META, span, message);
        return;
    }

    let complementary = (is_select(cur.call.name().as_slice())
        && prev.call.name().as_slice() == b"reject")
        || (cur.call.name().as_slice() == b"reject" && is_select(prev.call.name().as_slice()));
    let (select_var, reject_var, partition_call) = if complementary {
        complementary_variables(&prev, prev_stmt, &cur, cur_stmt)
    } else {
        negation_partition_vars(ctx, &cur, cur_stmt, &prev, prev_stmt)
    };

    let partition_text = build_partition_call(ctx, &partition_call);
    let select_var_src = String::from_utf8_lossy(ctx.text(select_var));
    let reject_var_src = String::from_utf8_lossy(ctx.text(reject_var));
    let replacement = format!("{select_var_src}, {reject_var_src} = {partition_text}");

    let edits = vec![
        Edit::replace(prev_stmt.span(), replacement.into_bytes()),
        Edit::delete(ctx.whole_lines(cur_stmt.span())),
    ];
    ctx.report_with_fix(
        &PartitionInsteadOfDoubleSelect::META,
        span,
        message,
        Fix { applicability: Applicability::Unsafe, edits },
    );
}
