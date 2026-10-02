//! `Lint/UnmodifiedReduceAccumulator`, ported from RuboCop's
//! `lib/rubocop/cop/lint/unmodified_reduce_accumulator.rb`.
//!
//! # `reduce`/`inject` with any receiver, any explicit-vs-numbered/it block
//!
//! Upstream dispatches through three separate hooks (`on_block`,
//! `on_numblock`, aliased `on_itblock`) because whitequark gives each block
//! shape its own node type. Prism represents all three uniformly as a
//! [`NodeKind::CallNode`] whose [`CallNode::block`] is a single
//! [`NodeKind::BlockNode`] (only its own `parameters()` differ: a real
//! `BlockParametersNode`, a `NumberedParametersNode`, or an
//! `ItParametersNode`), so this rule subscribes to [`NodeKind::CallNode`]
//! directly and checks `call.name()` against `reduce`/`inject` itself
//! (upstream's `reduce_with_block?` node-pattern receiver is an
//! unconstrained wildcard, so no receiver check is needed either).
//! [`block_arg_names`] resolves accumulator/element names for all three
//! parameter shapes: a `NumberedParametersNode` with `maximum >= 2` yields
//! literal `_1`/`_2` (Ruby defines every numbered parameter up to
//! `maximum`); `maximum < 2` or an `ItParametersNode` (always arity 1, so
//! `it` alone can never supply both an accumulator and an element) mirror
//! upstream's `node.argument_list.length >= 2` bailout by returning `None`.
//!
//! # `argument_list` flattening for destructured block parameters
//!
//! RuboCop-AST's `argument_list` flattens a destructured parameter
//! (`|acc, (el, index)|`, an `mlhs` node in whitequark) into its own
//! constituent names in position, so `argument_list[1]` is `el`, not the
//! `mlhs` node. Prism's equivalent is a [`NodeKind::MultiTargetNode`]
//! nested inside `requireds`/`posts`; [`flatten_param_names`] walks
//! `requireds`, `optionals`, `rest`, `posts`, `keywords`, `keyword_rest`,
//! and the block parameter in that order (the only order Ruby's grammar
//! permits them to appear in), recursing into any `MultiTargetNode`'s own
//! `lefts`/`rest`/`rights` the same way.
//!
//! # `return_values`: next/break only at the block's own nesting level
//!
//! [`collect_next_break`] reproduces upstream's `n.each_ancestor(:any_block
//! ).first != block_body_node.parent` filter (only a `next`/`break` whose
//! nearest enclosing block is *this* block counts) by threading the span
//! of the nearest enclosing [`NodeKind::BlockNode`] through the recursive
//! walk, updating it on entry to any nested `BlockNode` (numbered/it/
//! explicit alike) and comparing against the outer block's own span at
//! each `next`/`break` found.
//!
//! # `acceptable_return?`/`expression_values`: a deliberate false-negative
//!
//! Per the upstream class doc, this cop only flags a return value whose
//! entire expression tree references the element and *no other* variable
//! (recursively: every [`NodeKind::LocalVariableReadNode`]/ivar/gvar/cvar
//! read or write, zero-argument method call, or shorthand-assignment
//! target found anywhere in its subtree). The presence of any other
//! variable, or any zero-arg call (upstream's `$(send _ _)`, which only
//! has an arity-2 shape -- receiver and method, no arguments at all --
//! captures the whole node as an opaque, non-symbol value), makes the
//! expression ambiguous and the return value is left unflagged by design,
//! matching the class doc's own false-negative-biased examples (`"good,
//! recursive" keys.reduce(self) { |result, key| result[key] }`-style and
//! the two-interpolation-heredoc fixture, where the accumulator's own name
//! appearing anywhere alongside the element's is enough to suppress the
//! offense even though the heredoc itself is never otherwise "modified").

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::{ArgumentsNode, BlockNode, CallNode, ParametersNode};
use ruby_ast::{for_each_child, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Ensure the accumulator `{accum}` will be modified by `{method}`.";
/// RuboCop's `MSG_INDEX`.
const MSG_INDEX: &str = "Do not return an element of the accumulator in `{method}`.";

/// Checks for `reduce` or `inject` blocks that do not update the accumulator each iteration.
#[derive(Debug, Clone)]
pub struct UnmodifiedReduceAccumulator;

impl Rule for UnmodifiedReduceAccumulator {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UnmodifiedReduceAccumulator",
        department: Department::Lint,
        summary: "Checks for `reduce` or `inject` blocks that do not update the accumulator each iteration.",
        explanation: "\
Looks for `reduce` or `inject` blocks where the value returned (implicitly or
explicitly) does not include the accumulator. A block is considered valid as
long as at least one return value includes the accumulator.

If the accumulator is not included in the return value, then the entire
block will just return a transformation of the last element value, and
could be rewritten as such without a loop.

Also catches instances where an index of the accumulator is returned, as
this may change the type of object being retained.

NOTE: For the purpose of reducing false positives, this cop only flags
returns in `reduce` blocks where the element is the only variable in
the expression (since we will not be able to tell what other variables
relate to via static analysis).

```ruby
# bad
(1..4).reduce(0) do |acc, el|
  el * 2
end

# bad, may raise a NoMethodError after the first iteration
%w(a b c).reduce({}) do |acc, letter|
  acc[letter] = true
end

# good
(1..4).reduce(0) do |acc, el|
  acc + el * 2
end

# good, element is returned but modified using the accumulator
values.reduce do |acc, el|
  el << acc
  el
end

# good, returns the accumulator instead of the index
%w(a b c).reduce({}) do |acc, letter|
  acc[letter] = true
  acc
end

# good, at least one branch returns the accumulator
values.reduce(nil) do |result, value|
  break result if something?
  value
end

# good, recursive
keys.reduce(self) { |result, key| result[key] }

# ignored as the return value cannot be determined
enum.reduce do |acc, el|
  x = foo(acc, el)
  bar(x)
end
```",
        enabled_by_default: false,
        severity: Severity::Warning,
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
        let name = call.name().as_slice();
        if name != b"reduce" && name != b"inject" {
            return;
        }
        let Some(block_raw) = call.block() else { return };
        let Some(block) = block_raw.as_block_node() else { return };
        let Some(body) = block.body() else { return };
        let Some((accum, element)) = block_arg_names(&block) else { return };
        check_return_values(ctx, &call, &block, &body, &accum, &element);
    }
}

/// RuboCop's `check_return_values`.
fn check_return_values(
    ctx: &mut Context<'_>,
    call: &CallNode<'_>,
    block: &BlockNode<'_>,
    body: &Node<'_>,
    accum: &[u8],
    element: &[u8],
) {
    let return_values = collect_return_values(block, body);
    let method = call.name().as_slice();

    if let Some(target) = returned_accumulator_index(&return_values, accum, element) {
        ctx.report(&UnmodifiedReduceAccumulator::META, target.span(), format_index_msg(method));
        return;
    }

    if potential_offense(&return_values, body, element, accum) {
        for val in &return_values {
            if !acceptable_return(val, element) {
                ctx.report(
                    &UnmodifiedReduceAccumulator::META,
                    val.span(),
                    format_msg(accum, method),
                );
            }
        }
    }
}

fn format_msg(accum: &[u8], method: &[u8]) -> String {
    MSG.replacen("{accum}", &String::from_utf8_lossy(accum), 1).replacen(
        "{method}",
        &String::from_utf8_lossy(method),
        1,
    )
}

fn format_index_msg(method: &[u8]) -> String {
    MSG_INDEX.replacen("{method}", &String::from_utf8_lossy(method), 1)
}

/// RuboCop's `block_arg_name(node, 0)`/`block_arg_name(node, 1)`, covering
/// all three Prism parameter shapes. `None` means "not enough block
/// arguments", matching upstream's `node.argument_list.length >= 2` guard.
fn block_arg_names(block: &BlockNode<'_>) -> Option<(Vec<u8>, Vec<u8>)> {
    let params = block.parameters()?;
    match params.kind() {
        NodeKind::NumberedParametersNode => {
            let np = params.as_numbered_parameters_node().expect("kind matched");
            if np.maximum() < 2 {
                return None;
            }
            Some((b"_1".to_vec(), b"_2".to_vec()))
        }
        NodeKind::BlockParametersNode => {
            let bp = params.as_block_parameters_node().expect("kind matched");
            let p = bp.parameters()?;
            let names = flatten_param_names(&p);
            if names.len() < 2 {
                return None;
            }
            Some((names[0].clone(), names[1].clone()))
        }
        // `ItParametersNode` (always arity 1: `it` alone can't supply
        // both an accumulator and an element) falls into this wildcard.
        _ => None,
    }
}

/// RuboCop-AST's `argument_list`, flattening destructured parameters.
fn flatten_param_names(params: &ParametersNode<'_>) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for n in &params.requireds() {
        flatten_one(&n, &mut out);
    }
    for n in &params.optionals() {
        flatten_one(&n, &mut out);
    }
    if let Some(r) = params.rest() {
        flatten_one(&r, &mut out);
    }
    for n in &params.posts() {
        flatten_one(&n, &mut out);
    }
    for n in &params.keywords() {
        flatten_one(&n, &mut out);
    }
    if let Some(kr) = params.keyword_rest() {
        flatten_one(&kr, &mut out);
    }
    if let Some(b) = params.block() {
        flatten_one(&b.as_node(), &mut out);
    }
    out
}

fn flatten_one(node: &Node<'_>, out: &mut Vec<Vec<u8>>) {
    match node.kind() {
        NodeKind::RequiredParameterNode => {
            out.push(
                node.as_required_parameter_node().expect("kind matched").name().as_slice().to_vec(),
            );
        }
        NodeKind::OptionalParameterNode => {
            out.push(
                node.as_optional_parameter_node().expect("kind matched").name().as_slice().to_vec(),
            );
        }
        NodeKind::RestParameterNode => {
            if let Some(n) = node.as_rest_parameter_node().expect("kind matched").name() {
                out.push(n.as_slice().to_vec());
            }
        }
        NodeKind::RequiredKeywordParameterNode => {
            out.push(
                node.as_required_keyword_parameter_node()
                    .expect("kind matched")
                    .name()
                    .as_slice()
                    .to_vec(),
            );
        }
        NodeKind::OptionalKeywordParameterNode => {
            out.push(
                node.as_optional_keyword_parameter_node()
                    .expect("kind matched")
                    .name()
                    .as_slice()
                    .to_vec(),
            );
        }
        NodeKind::KeywordRestParameterNode => {
            if let Some(n) = node.as_keyword_rest_parameter_node().expect("kind matched").name() {
                out.push(n.as_slice().to_vec());
            }
        }
        NodeKind::BlockParameterNode => {
            if let Some(n) = node.as_block_parameter_node().expect("kind matched").name() {
                out.push(n.as_slice().to_vec());
            }
        }
        NodeKind::MultiTargetNode => {
            let mt = node.as_multi_target_node().expect("kind matched");
            for l in &mt.lefts() {
                flatten_one(&l, out);
            }
            if let Some(r) = mt.rest() {
                flatten_one(&r, out);
            }
            for r in &mt.rights() {
                flatten_one(&r, out);
            }
        }
        _ => {}
    }
}

/// RuboCop's `return_values`: the block body's last statement (or its only
/// statement/expression), plus the first argument of every `next`/`break`
/// at the block's own nesting level.
fn collect_return_values<'pr>(block: &BlockNode<'pr>, body: &Node<'pr>) -> Vec<Node<'pr>> {
    let mut out = Vec::new();
    if let Some(stmts) = body.as_statements_node() {
        if let Some(last) = stmts.body().iter().last() {
            out.push(last);
        }
    } else {
        out.push(*body);
    }
    let block_span = block.as_node().span();
    collect_next_break(body, block_span, block_span, &mut out);
    out
}

/// RuboCop's `n.each_ancestor(:any_block).first != block_body_node.parent`
/// filter: `current` is the span of the nearest enclosing `BlockNode` seen
/// so far during the walk, updated on entering a nested one.
fn collect_next_break<'pr>(node: &Node<'pr>, outer: Span, current: Span, out: &mut Vec<Node<'pr>>) {
    match node.kind() {
        NodeKind::NextNode => {
            if current == outer {
                let n = node.as_next_node().expect("kind matched");
                if let Some(v) = first_argument(n.arguments()) {
                    out.push(v);
                }
            }
            for_each_child(node, |c| collect_next_break(c, outer, current, out));
        }
        NodeKind::BreakNode => {
            if current == outer {
                let n = node.as_break_node().expect("kind matched");
                if let Some(v) = first_argument(n.arguments()) {
                    out.push(v);
                }
            }
            for_each_child(node, |c| collect_next_break(c, outer, current, out));
        }
        NodeKind::BlockNode => {
            let new_current = node.span();
            for_each_child(node, |c| collect_next_break(c, outer, new_current, out));
        }
        _ => for_each_child(node, |c| collect_next_break(c, outer, current, out)),
    }
}

fn first_argument(args: Option<ArgumentsNode<'_>>) -> Option<Node<'_>> {
    args.and_then(|a| a.arguments().iter().next())
}

/// RuboCop's `accumulator_index?` plus `returned_accumulator_index`: a
/// return value that is itself `acc[...]`/`acc[...]=`, always flagged for
/// `[]=`, and flagged for `[]` unless one of its index arguments uses the
/// element (presumably intentional).
fn returned_accumulator_index<'pr>(
    return_values: &[Node<'pr>],
    accum: &[u8],
    element: &[u8],
) -> Option<Node<'pr>> {
    for val in return_values {
        let Some(call) = val.as_call_node() else { continue };
        let Some(receiver) = call.receiver() else { continue };
        let Some(recv_lvar) = receiver.as_local_variable_read_node() else { continue };
        if recv_lvar.name().as_slice() != accum {
            continue;
        }
        let name = call.name().as_slice();
        if name != b"[]" && name != b"[]=" {
            continue;
        }
        if name == b"[]=" {
            return Some(*val);
        }
        let uses_element = call
            .arguments()
            .is_some_and(|args| args.arguments().iter().any(|a| lvar_used(&a, element)));
        if !uses_element {
            return Some(*val);
        }
    }
    None
}

/// RuboCop's `potential_offense?`.
fn potential_offense(
    return_values: &[Node<'_>],
    body: &Node<'_>,
    element: &[u8],
    accum: &[u8],
) -> bool {
    !(element_modified(body, element) || returns_accumulator_anywhere(return_values, accum))
}

/// RuboCop's `returns_accumulator_anywhere?`.
fn returns_accumulator_anywhere(return_values: &[Node<'_>], accum: &[u8]) -> bool {
    return_values.iter().any(|v| lvar_used(v, accum))
}

/// RuboCop's `lvar_used?` node-pattern matcher: a direct (non-recursive)
/// match against a single node -- a bare read, a plain assignment, a
/// `var << ...` call, or a single-interpolation string, all naming
/// `name`. Upstream's pattern also lists a shorthand-assignment
/// alternative (`(SHORTHAND_ASSIGNMENTS (lvasgn %1))`), but that
/// sub-pattern has no trailing `...` and so requires the `op_asgn`/
/// `or_asgn`/`and_asgn` node to have exactly one child -- real shorthand-assignment
/// nodes always have two or three (target, [operator,] value) -- making
/// it dead code that never matches in the actual gem (confirmed against
/// a real `rubocop` 1.91.0 process: `lvar_used?` returns `nil` for
/// `acc += 1`/`acc ||= 1`/`acc &&= 1` alike), so it is omitted here too.
fn lvar_used(node: &Node<'_>, name: &[u8]) -> bool {
    match node.kind() {
        NodeKind::LocalVariableReadNode => {
            node.as_local_variable_read_node().expect("kind matched").name().as_slice() == name
        }
        NodeKind::LocalVariableWriteNode => {
            node.as_local_variable_write_node().expect("kind matched").name().as_slice() == name
        }
        NodeKind::CallNode => {
            let call = node.as_call_node().expect("kind matched");
            call.name().as_slice() == b"<<"
                && call
                    .receiver()
                    .and_then(|r| r.as_local_variable_read_node())
                    .is_some_and(|l| l.name().as_slice() == name)
        }
        NodeKind::InterpolatedStringNode => single_interpolation_of(node, name),
        _ => false,
    }
}

/// Whether `node` (an `InterpolatedStringNode`) is exactly one
/// interpolation, `#{<lvar name>}`, with no surrounding literal text --
/// RuboCop's `(dstr (begin (lvar %1)))`.
fn single_interpolation_of(node: &Node<'_>, name: &[u8]) -> bool {
    let s = node.as_interpolated_string_node().expect("kind matched");
    let parts: Vec<Node<'_>> = s.parts().iter().collect();
    let [part] = parts.as_slice() else { return false };
    let Some(embedded) = part.as_embedded_statements_node() else { return false };
    let Some(stmts) = embedded.statements() else { return false };
    let body: Vec<Node<'_>> = stmts.body().iter().collect();
    let [single] = body.as_slice() else { return false };
    single.as_local_variable_read_node().is_some_and(|l| l.name().as_slice() == name)
}

/// RuboCop's `element_modified?` (`def_node_search`): a deep search over
/// `root` (including `root` itself) for any node directly matching one of
/// the four modification shapes.
fn element_modified(root: &Node<'_>, element: &[u8]) -> bool {
    if modifies_element(root, element) {
        return true;
    }
    let mut found = false;
    ruby_ast::each_descendant(root, &mut |n| {
        if !found && modifies_element(n, element) {
            found = true;
        }
    });
    found
}

fn modifies_element(node: &Node<'_>, element: &[u8]) -> bool {
    match node.kind() {
        NodeKind::CallNode => {
            let call = node.as_call_node().expect("kind matched");
            let name = call.name().as_slice();
            let branch1 = name != b"[]"
                && name != b"[]="
                && call.arguments().is_some_and(|args| {
                    let items: Vec<Node<'_>> = args.arguments().iter().collect();
                    items.len() >= 2 && items.iter().any(|a| subtree_has_lvar(a, element))
                });
            let branch2 = call
                .receiver()
                .and_then(|r| r.as_local_variable_read_node())
                .is_some_and(|l| l.name().as_slice() == element)
                && call
                    .arguments()
                    .is_some_and(|args| args.arguments().iter().any(|a| is_variable_or_call(&a)));
            branch1 || branch2
        }
        NodeKind::LocalVariableWriteNode => {
            node.as_local_variable_write_node().expect("kind matched").name().as_slice() == element
        }
        NodeKind::LocalVariableOperatorWriteNode => {
            node.as_local_variable_operator_write_node().expect("kind matched").name().as_slice()
                == element
        }
        NodeKind::LocalVariableAndWriteNode => {
            node.as_local_variable_and_write_node().expect("kind matched").name().as_slice()
                == element
        }
        NodeKind::LocalVariableOrWriteNode => {
            node.as_local_variable_or_write_node().expect("kind matched").name().as_slice()
                == element
        }
        _ => false,
    }
}

fn is_variable_or_call(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::InstanceVariableReadNode
            | NodeKind::GlobalVariableReadNode
            | NodeKind::ClassVariableReadNode
            | NodeKind::LocalVariableReadNode
            | NodeKind::CallNode
    )
}

/// Whether `node`'s own subtree (including itself) contains a read of the
/// local variable `name` -- the backtick ("search descendant") half of
/// upstream's `element_modified?` first branch.
fn subtree_has_lvar(node: &Node<'_>, name: &[u8]) -> bool {
    if node.as_local_variable_read_node().is_some_and(|l| l.name().as_slice() == name) {
        return true;
    }
    let mut found = false;
    ruby_ast::each_descendant(node, &mut |n| {
        if !found && n.as_local_variable_read_node().is_some_and(|l| l.name().as_slice() == name) {
            found = true;
        }
    });
    found
}

/// RuboCop's `acceptable_return?`.
fn acceptable_return(return_val: &Node<'_>, element: &[u8]) -> bool {
    let mut vars = Vec::new();
    expression_values(return_val, &mut vars);
    if vars.is_empty() {
        return true;
    }
    vars.iter().any(|v| !matches!(v, Var::Named(n) if n == element))
}

/// A captured value from RuboCop's `expression_values` node-search: either
/// a variable/assignment-target name, or an opaque non-symbol capture
/// (upstream's `$(send _ _)` zero-arg-call or `masgn` branches), which --
/// since it can never equal the element's own name -- always makes the
/// surrounding expression "ambiguous" per `acceptable_return?`.
enum Var {
    Named(Vec<u8>),
    Other,
}

/// RuboCop's `expression_values` (`def_node_search`): collects every
/// matching node anywhere in `node`'s own subtree, including `node` itself.
fn expression_values(node: &Node<'_>, out: &mut Vec<Var>) {
    collect_expression_value(node, out);
    ruby_ast::each_descendant(node, &mut |n| collect_expression_value(n, out));
}

fn collect_expression_value(node: &Node<'_>, out: &mut Vec<Var>) {
    if let Some(name) = variable_name(node) {
        out.push(Var::Named(name.to_vec()));
        return;
    }
    match node.kind() {
        NodeKind::ConstantWriteNode | NodeKind::MultiWriteNode => out.push(Var::Other),
        NodeKind::CallNode => {
            let call = node.as_call_node().expect("kind matched");
            if call.arguments().is_none() {
                out.push(Var::Other);
            }
        }
        _ => {}
    }
}

/// The name of a local/instance/class/global variable read, write or
/// (operator/and/or) compound write.
fn variable_name<'pr>(node: &Node<'pr>) -> Option<&'pr [u8]> {
    let name = match node.kind() {
        NodeKind::LocalVariableReadNode => node.as_local_variable_read_node()?.name(),
        NodeKind::InstanceVariableReadNode => node.as_instance_variable_read_node()?.name(),
        NodeKind::GlobalVariableReadNode => node.as_global_variable_read_node()?.name(),
        NodeKind::ClassVariableReadNode => node.as_class_variable_read_node()?.name(),
        NodeKind::LocalVariableWriteNode => node.as_local_variable_write_node()?.name(),
        NodeKind::InstanceVariableWriteNode => node.as_instance_variable_write_node()?.name(),
        NodeKind::ClassVariableWriteNode => node.as_class_variable_write_node()?.name(),
        NodeKind::GlobalVariableWriteNode => node.as_global_variable_write_node()?.name(),
        NodeKind::LocalVariableOperatorWriteNode => {
            node.as_local_variable_operator_write_node()?.name()
        }
        NodeKind::LocalVariableAndWriteNode => node.as_local_variable_and_write_node()?.name(),
        NodeKind::LocalVariableOrWriteNode => node.as_local_variable_or_write_node()?.name(),
        NodeKind::InstanceVariableOperatorWriteNode => {
            node.as_instance_variable_operator_write_node()?.name()
        }
        NodeKind::InstanceVariableAndWriteNode => {
            node.as_instance_variable_and_write_node()?.name()
        }
        NodeKind::InstanceVariableOrWriteNode => node.as_instance_variable_or_write_node()?.name(),
        NodeKind::ClassVariableOperatorWriteNode => {
            node.as_class_variable_operator_write_node()?.name()
        }
        NodeKind::ClassVariableAndWriteNode => node.as_class_variable_and_write_node()?.name(),
        NodeKind::ClassVariableOrWriteNode => node.as_class_variable_or_write_node()?.name(),
        NodeKind::GlobalVariableOperatorWriteNode => {
            node.as_global_variable_operator_write_node()?.name()
        }
        NodeKind::GlobalVariableAndWriteNode => node.as_global_variable_and_write_node()?.name(),
        NodeKind::GlobalVariableOrWriteNode => node.as_global_variable_or_write_node()?.name(),
        _ => return None,
    };
    Some(name.as_slice())
}
