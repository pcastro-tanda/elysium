//! `Style/TallyMethod`, ported from RuboCop's
//! `lib/rubocop/cop/style/tally_method.rb`.
//!
//! # `&sym`/literal block uniformity
//!
//! Upstream's four node patterns distinguish `.group_by(&:itself)` (a plain
//! `send`) from `.group_by { |x| x }`/`.group_by { _1 }`/`.group_by { it }`
//! (a `block`/`numblock`/`itblock` wrapping that same `send`). Prism has no
//! such wrapping: whether a call carries `&:sym` or a literal block, it is
//! the *same* [`NodeKind::CallNode`] either way, with [`CallNode::block`]
//! holding a [`NodeKind::BlockArgumentNode`] (for `&:sym`) or a
//! [`NodeKind::BlockNode`] (for a literal block). So every receiver/call
//! this port inspects is uniformly a `CallNode`, and [`is_group_by`] checks
//! its own `block()` shape directly instead of matching on the node's own
//! kind.
//!
//! # `group_by_send_node`/`replacement_range`
//!
//! Upstream's `group_by_send_node` unwraps a `block`/`numblock` receiver
//! down to its own `send_node` so `replacement_range` can start counting
//! from the receiver's `loc.selector`; since a Prism `CallNode`'s span
//! already covers any block it carries, the receiver *is* that `send_node`
//! already -- no unwrapping step is needed, and the replacement range's own
//! end is simply the outer `transform_values` call's own span end (covering
//! through its attached block, if it has a literal one), matching upstream's
//! `end_node.source_range.end_pos` for both the `node`-as-`end_node` and
//! `block_node`-as-`end_node` call sites.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::node::BlockNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_EACH_WITH_OBJECT: &str = "Use `tally` instead of `each_with_object`.";
const MSG_GROUP_BY: &str = "Use `tally` instead of `group_by` and `transform_values`.";

/// `COUNTING_METHODS`.
fn is_counting_method(name: &[u8]) -> bool {
    matches!(name, b"count" | b"size" | b"length")
}

/// Prefer `Enumerable#tally` over manual counting patterns.
#[derive(Debug, Clone)]
pub struct TallyMethod {
    /// `minimum_target_ruby_version 2.7`.
    enabled: bool,
}

impl Rule for TallyMethod {
    const META: RuleMeta = RuleMeta {
        name: "Style/TallyMethod",
        department: Department::Style,
        summary: "Prefer `Enumerable#tally` over manual counting patterns.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { enabled: options.target_ruby_version() >= 2.7 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        match call.name().as_slice() {
            b"each_with_object" => check_each_with_object(node, ctx),
            b"transform_values" => check_transform_values(node, ctx),
            _ => {}
        }
    }
}

/// `(send (const {nil? cbase} :Hash) :new (int 0))`.
fn is_hash_new_zero(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.name().as_slice() != b"new" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    if !is_bare_or_toplevel_const(&receiver) || const_name(&receiver).as_deref() != Some("Hash") {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let args = args.arguments();
    if args.len() != 1 {
        return false;
    }
    args.first()
        .expect("len checked above")
        .as_integer_node()
        .is_some_and(|n| TryInto::<i32>::try_into(n.value()) == Ok(0))
}

/// The block's own body, narrowed to its (possibly empty) statement list.
fn statements<'pr>(block: &BlockNode<'pr>) -> Option<ruby_ast::node::NodeList<'pr>> {
    let body = block.body()?;
    Some(body.as_statements_node()?.body())
}

/// Whether `node` reads a block's parameter by name (a plain local
/// variable, a numbered `_1`, or an `it` implicit parameter).
fn reads_var(node: &Node<'_>, var_name: &[u8]) -> bool {
    match node.kind() {
        NodeKind::LocalVariableReadNode => {
            node.as_local_variable_read_node().is_some_and(|n| n.name().as_slice() == var_name)
        }
        NodeKind::ItLocalVariableReadNode => var_name == b"it",
        _ => false,
    }
}

/// RuboCop's `tally_each_with_object?`: exactly two block parameters (named
/// or numbered), whose body is a single `hash[elem] += 1` (the second
/// parameter indexed by the first, incremented by the integer literal `1`).
fn is_tally_each_with_object(block: &BlockNode<'_>) -> bool {
    let Some(params) = block.parameters() else { return false };
    let (elem, hash): (Vec<u8>, Vec<u8>) = match params.kind() {
        NodeKind::NumberedParametersNode => {
            let Some(n) = params.as_numbered_parameters_node() else { return false };
            if n.maximum() != 2 {
                return false;
            }
            (b"_1".to_vec(), b"_2".to_vec())
        }
        NodeKind::BlockParametersNode => {
            let Some(p) = params.as_block_parameters_node() else { return false };
            let Some(inner) = p.parameters() else { return false };
            if !inner.optionals().is_empty()
                || inner.rest().is_some()
                || !inner.posts().is_empty()
                || !inner.keywords().is_empty()
                || inner.keyword_rest().is_some()
                || inner.block().is_some()
            {
                return false;
            }
            let reqs = inner.requireds();
            if reqs.len() != 2 {
                return false;
            }
            let mut reqs = reqs.iter();
            let first = reqs.next().expect("len checked above");
            let second = reqs.next().expect("len checked above");
            let Some(first) = first.as_required_parameter_node() else { return false };
            let Some(second) = second.as_required_parameter_node() else { return false };
            (first.name().as_slice().to_vec(), second.name().as_slice().to_vec())
        }
        _ => return false,
    };

    let Some(stmts) = statements(block) else { return false };
    if stmts.len() != 1 {
        return false;
    }
    let stmt = stmts.first().expect("len checked above");
    let Some(write) = stmt.as_index_operator_write_node() else { return false };
    if write.binary_operator().as_slice() != b"+" {
        return false;
    }
    if write.value().as_integer_node().is_none_or(|n| TryInto::<i32>::try_into(n.value()) != Ok(1))
    {
        return false;
    }
    let Some(receiver) = write.receiver() else { return false };
    if !reads_var(&receiver, &hash) {
        return false;
    }
    let Some(args) = write.arguments() else { return false };
    let args = args.arguments();
    if args.len() != 1 {
        return false;
    }
    reads_var(&args.first().expect("len checked above"), &elem)
}

/// RuboCop's `on_send` for `each_with_object`.
fn check_each_with_object(node: &Node<'_>, ctx: &mut Context<'_>) {
    let call = node.as_call_node().expect("kind matched");
    let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };

    let Some(args) = call.arguments() else { return };
    let args = args.arguments();
    if args.len() != 1 {
        return;
    }
    if !is_hash_new_zero(&args.first().expect("len checked above")) {
        return;
    }
    if !is_tally_each_with_object(&block) {
        return;
    }

    let Some(message_loc) = call.message_loc() else { return };
    let selector_span = message_loc.span();
    let range = Span::new(selector_span.start, node.span().end);
    let fix = Fix {
        applicability: Applicability::Unsafe,
        edits: vec![Edit::replace(range, b"tally".to_vec())],
    };
    ctx.report_with_fix(&TallyMethod::META, selector_span, MSG_EACH_WITH_OBJECT, fix);
}

/// `(call _ :group_by (block_pass (sym :itself)))`.
fn is_group_by_block_pass_itself(call: &ruby_ast::node::CallNode<'_>) -> bool {
    let Some(block) = call.block() else { return false };
    let Some(block_arg) = block.as_block_argument_node() else { return false };
    let Some(expr) = block_arg.expression() else { return false };
    expr.as_symbol_node().is_some_and(|s| s.unescaped() == b"itself")
}

/// `(block (call _ :group_by) (args (arg _x)) (lvar _x))` /
/// `(numblock (call _ :group_by) 1 (lvar :_1))` /
/// `(itblock (call _ :group_by) :it (lvar :it))`.
fn is_group_by_identity_block(call: &ruby_ast::node::CallNode<'_>) -> bool {
    let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return false };
    let Some(var_name) = block_var_name(&block) else { return false };
    let Some(stmts) = statements(&block) else { return false };
    if stmts.len() != 1 {
        return false;
    }
    reads_var(&stmts.first().expect("len checked above"), &var_name)
}

/// The single identity-block parameter's own name (a required positional
/// parameter, a `maximum == 1` numbered block's `_1`, or an `itblock`'s
/// `it`); `None` for any other arity/shape.
fn block_var_name(block: &BlockNode<'_>) -> Option<Vec<u8>> {
    let params = block.parameters()?;
    match params.kind() {
        NodeKind::NumberedParametersNode => {
            let n = params.as_numbered_parameters_node()?;
            (n.maximum() == 1).then(|| b"_1".to_vec())
        }
        NodeKind::ItParametersNode => Some(b"it".to_vec()),
        NodeKind::BlockParametersNode => {
            let p = params.as_block_parameters_node()?;
            let inner = p.parameters()?;
            if inner.requireds().len() == 1
                && inner.optionals().is_empty()
                && inner.rest().is_none()
                && inner.posts().is_empty()
                && inner.keywords().is_empty()
                && inner.keyword_rest().is_none()
                && inner.block().is_none()
            {
                let req = inner.requireds().first()?;
                Some(req.as_required_parameter_node()?.name().as_slice().to_vec())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// `(call _ :group_by (block_pass (sym :itself)))` or an identity block
/// shape, either way named `group_by`.
fn is_group_by(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.name().as_slice() != b"group_by" {
        return false;
    }
    is_group_by_block_pass_itself(&call) || is_group_by_identity_block(&call)
}

/// `(block_pass (sym %COUNTING_METHODS))` on `node` itself.
fn has_counting_block_pass(call: &ruby_ast::node::CallNode<'_>) -> bool {
    let Some(block) = call.block() else { return false };
    let Some(block_arg) = block.as_block_argument_node() else { return false };
    let Some(expr) = block_arg.expression() else { return false };
    expr.as_symbol_node().is_some_and(|s| is_counting_method(s.unescaped()))
}

/// A literal `transform_values` block whose single identity parameter's
/// body is `(send (lvar _v) %COUNTING_METHODS)`.
fn has_counting_transform_block(call: &ruby_ast::node::CallNode<'_>) -> bool {
    let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return false };
    let Some(var_name) = block_var_name(&block) else { return false };
    let Some(stmts) = statements(&block) else { return false };
    if stmts.len() != 1 {
        return false;
    }
    let Some(send) = stmts.first().expect("len checked above").as_call_node() else { return false };
    if send.arguments().is_some() || send.block().is_some() {
        return false;
    }
    if !is_counting_method(send.name().as_slice()) {
        return false;
    }
    let Some(receiver) = send.receiver() else { return false };
    reads_var(&receiver, &var_name)
}

/// RuboCop's `on_send` for `transform_values`.
fn check_transform_values(node: &Node<'_>, ctx: &mut Context<'_>) {
    let call = node.as_call_node().expect("kind matched");
    let Some(receiver) = call.receiver() else { return };
    if !is_group_by(&receiver) {
        return;
    }
    if !(has_counting_block_pass(&call) || has_counting_transform_block(&call)) {
        return;
    }

    let group_by_call = receiver.as_call_node().expect("is_group_by matched a CallNode");
    let Some(message_loc) = group_by_call.message_loc() else { return };
    let selector_span = message_loc.span();
    let range = Span::new(selector_span.start, node.span().end);
    let fix = Fix {
        applicability: Applicability::Unsafe,
        edits: vec![Edit::replace(range, b"tally".to_vec())],
    };
    ctx.report_with_fix(&TallyMethod::META, selector_span, MSG_GROUP_BY, fix);
}
