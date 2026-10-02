//! `Style/ReduceToHash`, ported from RuboCop's
//! `lib/rubocop/cop/style/reduce_to_hash.rb`.
//!
//! whitequark's `numblock` (bare `_1`/`_2`, never declared) is Prism's
//! [`NodeKind::NumberedParametersNode`]; the upstream patterns hardcode which
//! numbered parameter is the accumulator and which is the element
//! (`each_with_object` yields element-then-accumulator, so `_2` is the
//! accumulator; `inject`/`reduce` yields accumulator-then-element, so `_1`
//! is), which [`each_with_object_to_hash`] and [`inject_to_hash`] check
//! directly rather than through a matcher DSL. whitequark's `(begin stmt1
//! stmt2)` (a two-statement, non-elided body) is simply a two-element
//! `StatementsNode::body()` in Prism (which always wraps a body, singular or
//! not).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, CallNode, ParametersNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `to_h { ... }` instead of `%<method>s`.";

/// A matched "bad" hash-building transformation: the key/value expressions
/// computed from the element, and the accumulator (hash) variable's name
/// (used to detect accumulator leakage into the key/value expressions).
struct Match<'pr> {
    key: Node<'pr>,
    value: Node<'pr>,
    accumulator_name: Vec<u8>,
}

/// Use `to_h { ... }` instead of `each_with_object`, `inject`, or `reduce`
/// to build a hash.
#[derive(Debug, Clone)]
pub struct ReduceToHash {
    /// `minimum_target_ruby_version 2.6`.
    enabled: bool,
}

impl Rule for ReduceToHash {
    const META: RuleMeta = RuleMeta {
        name: "Style/ReduceToHash",
        department: Department::Style,
        summary: "Use `to_h { ... }` instead of `each_with_object`, `inject`, or `reduce` to \
            build a hash.",
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
        Ok(Self { enabled: options.target_ruby_version() >= 2.6 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled {
            return;
        }
        let call = node.as_call_node().expect("kind matched");
        let is_each_with_object = call.name().as_slice() == b"each_with_object";
        if !is_each_with_object && !matches!(call.name().as_slice(), b"inject" | b"reduce") {
            return;
        }
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };

        let Some(m) = (if is_each_with_object {
            each_with_object_to_hash(&call, &block)
        } else {
            inject_to_hash(&call, &block)
        }) else {
            return;
        };

        if self_or_descendant_lvar_ref(&m.key, &m.accumulator_name)
            || self_or_descendant_lvar_ref(&m.value, &m.accumulator_name)
        {
            return;
        }
        if nested_match(&m.key) || nested_match(&m.value) {
            return;
        }

        register_offense(ctx, &call, &block, &m, is_each_with_object);
    }
}

/// Is `node` itself a read of local variable `name`?
fn is_lvar_named(node: &Node<'_>, name: &[u8]) -> bool {
    node.as_local_variable_read_node().is_some_and(|lv| lv.name().as_slice() == name)
}

/// Does `node`, or any of its descendants, read local variable `name`?
fn self_or_descendant_lvar_ref(node: &Node<'_>, name: &[u8]) -> bool {
    if is_lvar_named(node, name) {
        return true;
    }
    let mut found = false;
    ruby_ast::each_descendant(node, &mut |child| {
        if is_lvar_named(child, name) {
            found = true;
        }
    });
    found
}

/// A plain (non-destructured) required parameter's name, or `None` if
/// `node` is not one.
fn required_param_name(node: &Node<'_>) -> Option<Vec<u8>> {
    Some(node.as_required_parameter_node()?.name().as_slice().to_vec())
}

/// `(args (arg $_) (arg $_))`: a block's `ParametersNode` holding exactly
/// two plain required parameters and nothing else.
fn two_plain_required<'pr>(parameters: &ParametersNode<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
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
    let [first, second] = requireds.as_slice() else { return None };
    Some((*first, *second))
}

/// The block's exactly-two plain required parameters, if its
/// `BlockParametersNode` has that shape.
fn named_params<'pr>(block: &BlockNode<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
    let params = block.parameters()?;
    let block_params = params.as_block_parameters_node()?;
    two_plain_required(&block_params.parameters()?)
}

/// `(hash)`: a call's sole argument is a hash literal with zero elements.
fn has_empty_hash_arg(call: &CallNode<'_>) -> bool {
    let args: Vec<Node<'_>> = match call.arguments() {
        Some(a) => a.arguments().iter().collect(),
        None => return false,
    };
    let [arg] = args.as_slice() else { return false };
    arg.as_hash_node().is_some_and(|h| h.elements().is_empty())
}

/// The block body's sole statement, or `None` for an empty or
/// multi-statement body.
fn single_statement<'pr>(block: &BlockNode<'pr>) -> Option<Node<'pr>> {
    let stmts = block.body()?.as_statements_node()?;
    let items: Vec<Node<'_>> = stmts.body().iter().collect();
    let [stmt] = items.as_slice() else { return None };
    Some(*stmt)
}

/// The block body's exactly-two statements, or `None` otherwise.
fn two_statements<'pr>(block: &BlockNode<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
    let stmts = block.body()?.as_statements_node()?;
    let items: Vec<Node<'_>> = stmts.body().iter().collect();
    let [first, second] = items.as_slice() else { return None };
    Some((*first, *second))
}

/// The `hash[key] = value` statement's captured key/value, if `stmt` has
/// that shape and its receiver reads local variable `hash_name`.
fn match_index_assign<'pr>(stmt: &Node<'pr>, hash_name: &[u8]) -> Option<(Node<'pr>, Node<'pr>)> {
    let call = stmt.as_call_node()?;
    if call.name().as_slice() != b"[]=" {
        return None;
    }
    if !is_lvar_named(&call.receiver()?, hash_name) {
        return None;
    }
    let args: Vec<Node<'_>> = call.arguments()?.arguments().iter().collect();
    let [key, value] = args.as_slice() else { return None };
    Some((*key, *value))
}

/// `each_with_object_to_hash?`: `_.each_with_object({}) { |elem, hash|
/// hash[$key] = $value }` (named or numbered, `_2` being the accumulator for
/// numbered since `each_with_object` yields element-then-accumulator).
fn each_with_object_to_hash<'pr>(
    call: &CallNode<'pr>,
    block: &BlockNode<'pr>,
) -> Option<Match<'pr>> {
    if !has_empty_hash_arg(call) {
        return None;
    }
    let params = block.parameters()?;
    let hash_name = match params.kind() {
        NodeKind::BlockParametersNode => {
            let (elem, hash) = named_params(block)?;
            required_param_name(&elem)?;
            required_param_name(&hash)?
        }
        NodeKind::NumberedParametersNode => {
            if params.as_numbered_parameters_node()?.maximum() != 2 {
                return None;
            }
            b"_2".to_vec()
        }
        _ => return None,
    };
    let stmt = single_statement(block)?;
    let (key, value) = match_index_assign(&stmt, &hash_name)?;
    Some(Match { key, value, accumulator_name: hash_name })
}

/// `inject_to_hash?`: `_.{inject reduce}({}) { |hash, elem| hash[$key] =
/// $value; hash }` (named or numbered, `_1` being the accumulator for
/// numbered since `inject`/`reduce` yields accumulator-then-element).
fn inject_to_hash<'pr>(call: &CallNode<'pr>, block: &BlockNode<'pr>) -> Option<Match<'pr>> {
    if !has_empty_hash_arg(call) {
        return None;
    }
    let params = block.parameters()?;
    let hash_name = match params.kind() {
        NodeKind::BlockParametersNode => {
            let (hash, elem) = named_params(block)?;
            required_param_name(&elem)?;
            required_param_name(&hash)?
        }
        NodeKind::NumberedParametersNode => {
            if params.as_numbered_parameters_node()?.maximum() != 2 {
                return None;
            }
            b"_1".to_vec()
        }
        _ => return None,
    };
    let (first, second) = two_statements(block)?;
    let (key, value) = match_index_assign(&first, &hash_name)?;
    if !is_lvar_named(&second, &hash_name) {
        return None;
    }
    Some(Match { key, value, accumulator_name: hash_name })
}

/// RuboCop's `nested_match?`: does `node`, or any of its descendants,
/// contain a call to `each_with_object`/`inject`/`reduce` whose own block
/// also matches [`each_with_object_to_hash`]/[`inject_to_hash`]?
fn nested_match(node: &Node<'_>) -> bool {
    let mut found = false;
    let mut visit = |n: &Node<'_>| {
        if found {
            return;
        }
        let Some(call) = n.as_call_node() else { return };
        let is_each_with_object = call.name().as_slice() == b"each_with_object";
        if !is_each_with_object && !matches!(call.name().as_slice(), b"inject" | b"reduce") {
            return;
        }
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        let matched = if is_each_with_object {
            each_with_object_to_hash(&call, &block).is_some()
        } else {
            inject_to_hash(&call, &block).is_some()
        };
        if matched {
            found = true;
        }
    };
    visit(node);
    ruby_ast::each_descendant(node, &mut visit);
    found
}

/// RuboCop's `register_offense` + `replacement` + its helpers.
fn register_offense(
    ctx: &mut Context<'_>,
    call: &CallNode<'_>,
    block: &BlockNode<'_>,
    m: &Match<'_>,
    is_each_with_object: bool,
) {
    let Some(message_loc) = call.message_loc() else { return };
    let method_name = String::from_utf8_lossy(call.name().as_slice());
    let message = MSG.replacen("%<method>s", &method_name, 1);

    let numblock =
        matches!(block.parameters().map(|p| p.kind()), Some(NodeKind::NumberedParametersNode));
    let key_source = adjusted_source(ctx, &m.key, numblock, is_each_with_object);
    let value_source = adjusted_source(ctx, &m.value, numblock, is_each_with_object);
    let body = format!("[{key_source}, {value_source}]");

    let replacement = if numblock {
        if is_brace_block(ctx, block) {
            format!("to_h {{ {body} }}")
        } else {
            do_end_replacement(ctx, call, block, &body, None)
        }
    } else {
        named_block_replacement(ctx, call, block, &body, is_each_with_object)
    };

    let edit_span = Span::new(message_loc.span().start, block.as_node().span().end);
    ctx.report_with_fix(
        &ReduceToHash::META,
        message_loc.span(),
        message,
        Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(edit_span, replacement.into_bytes())],
        },
    );
}

/// RuboCop's `named_block_replacement`.
fn named_block_replacement(
    ctx: &Context<'_>,
    call: &CallNode<'_>,
    block: &BlockNode<'_>,
    body: &str,
    is_each_with_object: bool,
) -> String {
    let arg = element_arg_source(ctx, call, block, is_each_with_object);
    if is_brace_block(ctx, block) {
        format!("to_h {{ |{arg}| {body} }}")
    } else {
        do_end_replacement(ctx, call, block, body, Some(&arg))
    }
}

/// RuboCop's `element_arg_source`: the element parameter's own source --
/// the first param for `each_with_object` (element-then-accumulator), the
/// second for `inject`/`reduce` (accumulator-then-element).
fn element_arg_source(
    ctx: &Context<'_>,
    call: &CallNode<'_>,
    block: &BlockNode<'_>,
    is_each_with_object: bool,
) -> String {
    let (first, second) = named_params(block).expect("named block shape already matched");
    let _ = call;
    let elem = if is_each_with_object { first } else { second };
    String::from_utf8_lossy(ctx.text(elem.span())).into_owned()
}

/// RuboCop's `do_end_replacement`.
fn do_end_replacement(
    ctx: &Context<'_>,
    call: &CallNode<'_>,
    block: &BlockNode<'_>,
    body: &str,
    arg: Option<&str>,
) -> String {
    let _ = block;
    let args = arg.map_or(String::new(), |a| format!(" |{a}|"));
    let indent = indent(ctx, call);
    format!("to_h do{args}\n{indent}  {body}\n{indent}end")
}

/// `node.source_range.column`: the 0-based column where the whole call
/// expression (whitequark's block-node span, which starts at the
/// receiver) begins on its own line.
fn indent(ctx: &Context<'_>, call: &CallNode<'_>) -> String {
    let column = ctx.line_col(call.as_node().span().start).column;
    " ".repeat(column as usize)
}

/// Is the block delimited by `{...}` (as opposed to `do...end`)?
fn is_brace_block(ctx: &Context<'_>, block: &BlockNode<'_>) -> bool {
    ctx.text(block.opening_loc().span()) == b"{"
}

/// RuboCop's `adjusted_source`: for an `inject`/`reduce` numbered block, the
/// element is `_2`, but `to_h`'s single-implicit-param replacement block
/// only has one, so every literal `_2` substring becomes `_1`.
fn adjusted_source(
    ctx: &Context<'_>,
    expr: &Node<'_>,
    numblock: bool,
    is_each_with_object: bool,
) -> String {
    let source = String::from_utf8_lossy(ctx.text(expr.span())).into_owned();
    if !numblock || is_each_with_object {
        return source;
    }
    source.replace("_2", "_1")
}
