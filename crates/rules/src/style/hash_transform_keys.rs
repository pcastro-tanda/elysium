//! `Style/HashTransformKeys`, ported from RuboCop's
//! `lib/rubocop/cop/style/hash_transform_keys.rb` and its
//! `lib/rubocop/cop/mixin/hash_transform_method.rb` mixin (ported privately;
//! `Style/HashTransformValues` is a separate cop file and does not share
//! this code).
//!
//! Looks for `_.each_with_object({}) {...}`, `_.map {...}.to_h`,
//! `_.to_h {...}`, and `Hash[_.map {...}]` that only transform a hash's
//! keys, and suggests `transform_keys` instead. Ported without autocorrect
//! (`FixAvailability::None`): only offense detection is in scope.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::{CallNode, ParametersNode};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// A matched "bad" transformation: the key-argument's name, the expression
/// that computes the new key from it, and the value-argument's name (used
/// to detect a no-op transformation or one that reads both block args).
struct Match<'pr> {
    key_argname: Vec<u8>,
    key_expr: Node<'pr>,
    val_name: Vec<u8>,
}

/// `HashTransformMethod::array_receiver?`: `{(array ...) (send _
/// :each_with_index) (send _ :with_index _ ?) (send _ :zip ...)}`.
fn is_array_receiver(node: &Node<'_>) -> bool {
    if node.as_array_node().is_some() {
        return true;
    }
    let Some(call) = node.as_call_node() else { return false };
    let arg_count = call.arguments().map_or(0, |a| a.arguments().len());
    match call.name().as_slice() {
        b"each_with_index" => arg_count == 0,
        b"with_index" => arg_count <= 1,
        b"zip" => true,
        _ => false,
    }
}

/// Is `node` itself a read of local variable `name`?
fn is_lvar_named(node: &Node<'_>, name: &[u8]) -> bool {
    node.as_local_variable_read_node().is_some_and(|lv| lv.name().as_slice() == name)
}

/// Does any strict descendant of `node` (not `node` itself) read local
/// variable `name`? Backs `transformation_uses_both_args?` and
/// `use_transformed_argname?`.
fn descendant_lvar_ref(node: &Node<'_>, name: &[u8]) -> bool {
    let mut found = false;
    ruby_ast::each_descendant(node, &mut |child| {
        if is_lvar_named(child, name) {
            found = true;
        }
    });
    found
}

/// Does `node`, or any of its descendants, read local variable `name`?
/// Backs the `each_with_object` pattern's negated `` `_memo `` unification.
fn self_or_descendant_lvar_ref(node: &Node<'_>, name: &[u8]) -> bool {
    is_lvar_named(node, name) || descendant_lvar_ref(node, name)
}

/// A plain (non-destructured) required parameter's name, or `None` if
/// `node` is not one.
fn required_param_name(node: &Node<'_>) -> Option<Vec<u8>> {
    Some(node.as_required_parameter_node()?.name().as_slice().to_vec())
}

/// `(args (arg $_) (arg _val))`: a block's `ParametersNode` holding exactly
/// two plain required parameters and nothing else (no optionals, rest,
/// posts, keywords, or block arg).
fn two_plain_required_params(parameters: &ParametersNode<'_>) -> Option<(Vec<u8>, Vec<u8>)> {
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
    Some((required_param_name(first)?, required_param_name(second)?))
}

/// A block's `ParametersNode`, requiring the block to have exactly a plain
/// `BlockParametersNode` (no numbered/`it` params) with a `ParametersNode`.
fn block_parameters<'pr>(block_node: &Node<'pr>) -> Option<ParametersNode<'pr>> {
    let block = block_node.as_block_node()?;
    let params = block.parameters()?;
    let block_params = params.as_block_parameters_node()?;
    block_params.parameters()
}

/// `(args (arg $_) (arg _val)) (array $_ $(lvar _val))`: a block taking two
/// plain arguments whose single-statement body is a two-element array,
/// literally `[key_expr, (lvar val_name)]`. Shared by the `map`/`collect`
/// pattern and the `to_h` block pattern -- both have this exact shape.
fn match_two_arg_array_block<'pr>(block_node: &Node<'pr>) -> Option<(Vec<u8>, Vec<u8>, Node<'pr>)> {
    let block = block_node.as_block_node()?;
    let parameters = block_parameters(block_node)?;
    let (key_argname, val_name) = two_plain_required_params(&parameters)?;
    let body = block.body()?;
    let stmts = body.as_statements_node()?;
    let items: Vec<Node<'_>> = stmts.body().iter().collect();
    let [stmt] = items.as_slice() else { return None };
    let array = stmt.as_array_node()?;
    let elements: Vec<Node<'_>> = array.elements().iter().collect();
    let [key_expr, val_expr] = elements.as_slice() else { return None };
    if !is_lvar_named(val_expr, &val_name) {
        return None;
    }
    Some((key_argname, val_name, *key_expr))
}

/// `on_bad_each_with_object`:
/// `_.each_with_object({}) { |(k, v), h| h[$key_expr] = v }`, where
/// `$key_expr` (unlike the `map`/`to_h` variants) may be any expression, as
/// long as it does not itself read the accumulator.
fn match_each_with_object<'pr>(call: &CallNode<'pr>) -> Option<Match<'pr>> {
    if call.name().as_slice() != b"each_with_object" {
        return None;
    }
    let receiver = call.receiver()?;
    if is_array_receiver(&receiver) {
        return None;
    }
    let args: Vec<Node<'_>> = call.arguments()?.arguments().iter().collect();
    let [hash_arg] = args.as_slice() else { return None };
    if !hash_arg.as_hash_node()?.elements().is_empty() {
        return None;
    }

    let block_node = call.block()?;
    let block = block_node.as_block_node()?;
    let parameters = block_parameters(&block_node)?;
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
    let [pair, memo] = requireds.as_slice() else { return None };
    let memo_name = required_param_name(memo)?;

    let multi = pair.as_multi_target_node()?;
    if multi.rest().is_some() || !multi.rights().is_empty() {
        return None;
    }
    let lefts: Vec<Node<'_>> = multi.lefts().iter().collect();
    let [key_param, val_param] = lefts.as_slice() else { return None };
    let key_argname = required_param_name(key_param)?;
    let val_name = required_param_name(val_param)?;

    let body = block.body()?;
    let stmts = body.as_statements_node()?;
    let items: Vec<Node<'_>> = stmts.body().iter().collect();
    let [stmt] = items.as_slice() else { return None };
    let inner_call = stmt.as_call_node()?;
    if inner_call.name().as_slice() != b"[]=" {
        return None;
    }
    let inner_recv = inner_call.receiver()?;
    if !is_lvar_named(&inner_recv, &memo_name) {
        return None;
    }
    let inner_args: Vec<Node<'_>> = inner_call.arguments()?.arguments().iter().collect();
    let [key_expr, val_expr] = inner_args.as_slice() else { return None };
    if self_or_descendant_lvar_ref(key_expr, &memo_name) {
        return None;
    }
    if !is_lvar_named(val_expr, &val_name) {
        return None;
    }

    Some(Match { key_argname, key_expr: *key_expr, val_name })
}

/// `on_bad_to_h`, matched from `on_block`: `_.to_h { |k, v| [$key_expr, v] }`.
fn match_to_h_block<'pr>(call: &CallNode<'pr>) -> Option<Match<'pr>> {
    if call.name().as_slice() != b"to_h" {
        return None;
    }
    let receiver = call.receiver()?;
    if is_array_receiver(&receiver) {
        return None;
    }
    let block_node = call.block()?;
    let (key_argname, val_name, key_expr) = match_two_arg_array_block(&block_node)?;
    Some(Match { key_argname, key_expr, val_name })
}

/// `_.map {...}` / `_.collect {...}`, the shared core of the
/// `on_bad_hash_brackets_map` and `on_bad_map_to_h` patterns: `_.{map
/// collect} { |k, v| [$key_expr, v] }`.
fn match_map_block<'pr>(node: &Node<'pr>) -> Option<Match<'pr>> {
    let call = node.as_call_node()?;
    if !matches!(call.name().as_slice(), b"map" | b"collect") {
        return None;
    }
    let receiver = call.receiver()?;
    if is_array_receiver(&receiver) {
        return None;
    }
    let block_node = call.block()?;
    let (key_argname, val_name, key_expr) = match_two_arg_array_block(&block_node)?;
    Some(Match { key_argname, key_expr, val_name })
}

/// `on_bad_hash_brackets_map`: `Hash[_.{map collect} {...}]`.
fn match_hash_brackets_map<'pr>(call: &CallNode<'pr>) -> Option<Match<'pr>> {
    if call.name().as_slice() != b"[]" {
        return None;
    }
    let receiver = call.receiver()?;
    if ruby_ast::ext::const_name(&receiver).as_deref() != Some("Hash") {
        return None;
    }
    let args: Vec<Node<'_>> = call.arguments()?.arguments().iter().collect();
    let [inner] = args.as_slice() else { return None };
    match_map_block(inner)
}

/// `on_bad_map_to_h`, matched from `on_send`/`on_csend`: `_.{map
/// collect} {...}.to_h`, with no own arguments. Whitequark represents a
/// block-wrapped call as a `block` node around a plain `send`, so a `send`
/// visited on its own never carries block information; this cop's own
/// trailing block, if any (`{a: 1}.map {...}.to_h {...}`), is irrelevant to
/// the match and excluded from the reported span by [`report_map_to_h`].
fn match_map_to_h<'pr>(call: &CallNode<'pr>) -> Option<Match<'pr>> {
    if call.name().as_slice() != b"to_h" || call.arguments().is_some() {
        return None;
    }
    let receiver = call.receiver()?;
    match_map_block(&receiver)
}

/// `HashTransformMethod#handle_possible_offense`: filters out false
/// positives where the receiver most likely was not really a hash.
fn should_report(m: &Match<'_>) -> bool {
    // `noop_transformation?`
    if is_lvar_named(&m.key_expr, &m.key_argname) {
        return false;
    }
    // `transformation_uses_both_args?`
    if descendant_lvar_ref(&m.key_expr, &m.val_name) {
        return false;
    }
    // `use_transformed_argname?`
    descendant_lvar_ref(&m.key_expr, &m.key_argname)
}

/// Prefer `transform_keys` over `each_with_object`, `map`, or `to_h`.
#[derive(Debug, Clone)]
pub struct HashTransformKeys {
    /// Below Ruby 2.5, the cop is a no-op (`minimum_target_ruby_version
    /// 2.5`; `transform_keys` did not exist yet).
    enabled: bool,
    /// `return if target_ruby_version < 2.6` before `on_bad_to_h`.
    check_to_h_block: bool,
}

impl Rule for HashTransformKeys {
    const META: RuleMeta = RuleMeta {
        name: "Style/HashTransformKeys",
        department: Department::Style,
        summary: "Prefer `transform_keys` over `each_with_object`, `map`, or `to_h`.",
        explanation: "Looks for uses of `each_with_object({}) {...}`, `map {...}.to_h`, and \
            `Hash[map {...}]` that are actually just transforming the keys of a hash, and tries \
            to use a simpler & faster call to `transform_keys` instead.\n\nIt should only be \
            enabled on Ruby version 2.5 or newer (`transform_keys` was added in Ruby 2.5).\n\n\
            This cop is unsafe, as it can produce false positives if we are transforming an \
            enumerable of key-value-like pairs that isn't actually a hash, e.g.: \
            `[[k1, v1], [k2, v2], ...]`.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let target_ruby_version = options.target_ruby_version();
        Ok(Self {
            enabled: target_ruby_version >= 2.5,
            check_to_h_block: target_ruby_version >= 2.6,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled {
            return;
        }
        let call = node.as_call_node().expect("kind matched");

        if let Some(m) = match_each_with_object(&call) {
            if should_report(&m) {
                report(ctx, node.span(), "each_with_object");
            }
            return;
        }
        if self.check_to_h_block {
            if let Some(m) = match_to_h_block(&call) {
                if should_report(&m) {
                    report(ctx, node.span(), "to_h {...}");
                }
                return;
            }
        }
        if let Some(m) = match_hash_brackets_map(&call) {
            if should_report(&m) {
                report(ctx, node.span(), "Hash[_.map {...}]");
            }
            return;
        }
        if let Some(m) = match_map_to_h(&call) {
            if should_report(&m) {
                report(ctx, ruby_ast::ext::call_span_excluding_block(&call), "map {...}.to_h");
            }
        }
    }
}

fn report(ctx: &mut Context<'_>, span: ruby_source::Span, match_desc: &str) {
    ctx.report(
        &HashTransformKeys::META,
        span,
        format!("Prefer `transform_keys` over `{match_desc}`."),
    );
}
