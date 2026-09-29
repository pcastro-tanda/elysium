//! `Style/HashEachMethods`, ported from RuboCop's
//! `lib/rubocop/cop/style/hash_each_methods.rb` (its `AllowedReceivers` mixin
//! ported inline; see [`receiver_name`]).
//!
//! # Prism unifies whitequark's `send`/`csend`/`block`/`numblock`/`itblock`
//!
//! Upstream matches three independent shapes through three node-pattern
//! matchers (`kv_each`, `each_arguments`, `kv_each_with_block_pass`) plus a
//! separate `on_block_pass` visitor, because whitequark represents a bare
//! call, a call with a literal block, and a call with a `&:sym`/`&blk`
//! argument as different node *types* (`send`/`csend`, `block`/`numblock`/
//! `itblock`, and a `block_pass` argument node respectively). Prism's
//! `CallNode` covers all of these uniformly -- the receiver, method name,
//! arguments, and any attached block (literal or a `&`-passed value) are all
//! just fields of one node kind -- so this port subscribes to `CallNode`
//! alone and branches on `CallNode::block()`'s downcast
//! ([`Node::as_block_node`] vs [`Node::as_block_argument_node`]) instead of
//! four separate visitor methods.
//!
//! # Dead `Lint::UnusedArgument` override
//!
//! Upstream `include`s `Lint::UnusedArgument` (which normally drives
//! `after_leaving_scope` to add its own "unused argument" offenses) but then
//! completely overrides both methods that mixin relies on:
//! `check_argument` (here it just stashes the variable in `@block_args`,
//! never adding an offense) and `message` (here it takes three positional
//! arguments instead of the mixin's single `variable`, so the mixin's own
//! call site would raise if it were ever reached with the mixin's original
//! intent). The only other users of the resulting `@block_args`/`used?`
//! machinery are `correct_implicit`/`correct_args`, reachable solely from
//! `correct_key_value_each`'s `unless receiver` branch -- which never
//! triggers, because [`HashEachMethods::register_kv_offense`] already
//! guards on `target.receiver.receiver` being present before it is ever
//! called. All of that machinery is therefore unreachable dead code and is
//! not ported.
//!
//! # `hash_mutated?` structural equality
//!
//! Upstream's `` `(send %1 :[]= ...) `` backtick pattern compares the
//! mutated call's receiver against `root_receiver` with plain node `==`
//! (recursive structural equality, independent of source position). This
//! port approximates that with same-kind-and-same-source-text comparison
//! ([`same_receiver`]), which is exact for the plain-identifier/bare-call
//! receivers this cop actually sees in practice.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext;
use ruby_ast::node::{BlockNode, CallNode, ParametersNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `ARRAY_CONVERTER_METHODS`.
fn is_array_converter_method(name: &[u8]) -> bool {
    matches!(name, b"assoc" | b"chunk" | b"flatten" | b"rassoc" | b"sort" | b"sort_by" | b"to_a")
}

/// rubocop-ast's `LITERALS` (`Node#literal?`): every basic and composite
/// literal type. Prism's floating-point/rational/imaginary/range literals
/// stand in for whitequark's `float`/`rational`/`complex`/`irange`/`erange`.
fn is_literal_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::RangeNode
            | NodeKind::RationalNode
            | NodeKind::ImaginaryNode
    )
}

/// `use_array_converter_method_as_preceding?`: the `.each` call's own
/// receiver is itself a call to one of `ARRAY_CONVERTER_METHODS` (with or
/// without its own attached block -- Prism's `CallNode` covers both, unlike
/// whitequark's `type?(:call, :any_block)` alternative).
fn use_array_converter_method_as_preceding(call: &CallNode<'_>) -> bool {
    call.receiver()
        .and_then(|r| r.as_call_node())
        .is_some_and(|recv_call| is_array_converter_method(recv_call.name().as_slice()))
}

/// `root_receiver`: walks down the receiver chain (a `CallNode`'s own
/// `receiver` field, one step at a time) until it reaches a node that is
/// not itself a call with a receiver, and returns that node.
fn root_receiver<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let mut receiver = call.receiver()?;
    loop {
        match receiver.as_call_node().and_then(|c| c.receiver()) {
            Some(next) => receiver = next,
            None => return Some(receiver),
        }
    }
}

/// Same kind and same source text -- see the module doc's note on
/// `hash_mutated?`.
fn same_receiver(a: &Node<'_>, b: &Node<'_>, ctx: &Context<'_>) -> bool {
    a.kind() == b.kind() && ctx.text(a.span()) == ctx.text(b.span())
}

/// `hash_mutated?`: does any descendant of `call` (its whole subtree,
/// including its block body) write `root[...] = ...` where `root`
/// structurally matches `root_receiver`?
fn hash_mutated(call: &CallNode<'_>, root_receiver: &Node<'_>, ctx: &Context<'_>) -> bool {
    let mut mutated = false;
    ruby_ast::each_descendant(&call.as_node(), &mut |n| {
        if mutated {
            return;
        }
        if let Some(c) = n.as_call_node() {
            if c.name().as_slice() == b"[]="
                && c.receiver().is_some_and(|recv| same_receiver(&recv, root_receiver, ctx))
            {
                mutated = true;
            }
        }
    });
    mutated
}

/// `handleable?`.
fn handleable(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    if use_array_converter_method_as_preceding(call) {
        return false;
    }
    let Some(root) = root_receiver(call) else { return false };
    if hash_mutated(call, &root, ctx) {
        return false;
    }
    !is_literal_kind(root.kind()) || root.kind() == NodeKind::HashNode
}

/// `kv_each`'s inner `(call _ ${:keys :values})`, matched against `call`'s
/// receiver: an argumentless, block-less, receiverful call to `keys` or
/// `values`.
fn match_kv_each_receiver<'pr>(call: &CallNode<'pr>) -> Option<CallNode<'pr>> {
    let inner = call.receiver()?.as_call_node()?;
    if inner.receiver().is_none() || inner.arguments().is_some() || inner.block().is_some() {
        return None;
    }
    matches!(inner.name().as_slice(), b"keys" | b"values").then_some(inner)
}

/// `kv_each`: `$(call (call _ ${:keys :values}) :each)`, wrapped in a
/// literal block. `call` is already known to carry one; this only re-checks
/// the send shape (no own arguments) and the inner `keys`/`values` call.
fn match_kv_each<'pr>(call: &CallNode<'pr>) -> Option<CallNode<'pr>> {
    if call.arguments().is_some() {
        return None;
    }
    match_kv_each_receiver(call)
}

/// `kv_each_with_block_pass`: `(call $(call _ ${:keys :values}) :each
/// (block_pass (sym _)))` -- `call` already carries a `&:sym` block-pass
/// (checked by the caller); this only re-checks the send shape and the
/// inner `keys`/`values` call.
fn match_kv_each_block_pass<'pr>(call: &CallNode<'pr>) -> Option<CallNode<'pr>> {
    if call.arguments().is_some() {
        return None;
    }
    match_kv_each_receiver(call)
}

/// Ordered parameter list, reconstructed from `ParametersNode`'s
/// by-category fields in the fixed grammar order (required*, optional*,
/// rest?, post-required*, keyword*, keyword-rest?, block?) -- whitequark's
/// flat `args` node in effect, needed to reproduce `(args $_key $_value)`'s
/// "exactly two parameters, of any kind, in declaration order" match.
fn ordered_params<'pr>(parameters: &ParametersNode<'pr>) -> Vec<Node<'pr>> {
    let mut params: Vec<Node<'pr>> = Vec::new();
    params.extend(parameters.requireds().iter());
    params.extend(parameters.optionals().iter());
    if let Some(rest) = parameters.rest() {
        params.push(rest);
    }
    params.extend(parameters.posts().iter());
    params.extend(parameters.keywords().iter());
    if let Some(kwrest) = parameters.keyword_rest() {
        params.push(kwrest);
    }
    if let Some(block_param) = parameters.block() {
        params.push(block_param.as_node());
    }
    params
}

/// `each_arguments`: `(block (call _ :each)(args $_key $_value) ...)` --
/// requires a plain `BlockParametersNode` (excluding numbered/`it`-param
/// blocks, which have no such shape), with exactly two parameters total.
fn match_each_arguments<'pr>(
    call: &CallNode<'pr>,
    block: &BlockNode<'pr>,
) -> Option<(Node<'pr>, Node<'pr>)> {
    call.receiver()?;
    let params = block.parameters()?;
    let block_params = params.as_block_parameters_node()?;
    let parameters = block_params.parameters()?;
    match ordered_params(&parameters).as_slice() {
        [key, value] => Some((*key, *value)),
        _ => None,
    }
}

/// A parameter node's own name, stripped of a leading `*` (matching
/// upstream's `source.delete_prefix('*')`) by reading it straight off the
/// structured accessor instead of re-parsing text: `None` for an anonymous
/// `*` rest parameter (never referenceable, so never counted as "used").
fn param_name(node: &Node<'_>) -> Option<Vec<u8>> {
    if let Some(req) = node.as_required_parameter_node() {
        return Some(req.name().as_slice().to_vec());
    }
    if let Some(rest) = node.as_rest_parameter_node() {
        return rest.name().map(|n| n.as_slice().to_vec());
    }
    None
}

/// Every `LocalVariableReadNode`'s name under `body` (`node.body.each_descendant(:lvar)`).
fn collect_lvar_sources(body: &Node<'_>) -> Vec<Vec<u8>> {
    let mut sources = Vec::new();
    ruby_ast::each_descendant(body, &mut |n| {
        if let Some(lvar) = n.as_local_variable_read_node() {
            sources.push(lvar.name().as_slice().to_vec());
        }
    });
    sources
}

/// `block_arg.each_descendant(:arg, :restarg).all? { |d| lvar_sources.none?(...) }`:
/// every plain/rest parameter nested anywhere inside a destructured (`mlhs`)
/// block parameter is unreferenced.
fn mlhs_all_unused(node: &Node<'_>, lvar_sources: &[Vec<u8>]) -> bool {
    let mut all_unused = true;
    ruby_ast::each_descendant(node, &mut |n| {
        if let Some(name) = param_name(n) {
            if lvar_sources.iter().any(|s| s.as_slice() == name.as_slice()) {
                all_unused = false;
            }
        }
    });
    all_unused
}

/// `unused_block_arg_exist?`.
fn param_unused(node: &Node<'_>, lvar_sources: &[Vec<u8>]) -> bool {
    if node.as_multi_target_node().is_some() {
        return mlhs_all_unused(node, lvar_sources);
    }
    match param_name(node) {
        Some(name) => !lvar_sources.iter().any(|s| s.as_slice() == name.as_slice()),
        None => true,
    }
}

/// `(const {nil? cbase} :Name)`-ish check for `receiver_name`'s
/// `const_type?` guard: a plain or path constant reference.
fn is_const_node(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

/// `AllowedReceivers#receiver_name`. Only a `CallNode` ever has a receiver
/// at all (rubocop-ast's generic `Node#receiver` node-matcher only matches
/// a `send`/`csend`/block-wrapped-call shape); every other node kind falls
/// straight through to the `else` branch below (`receiver.source`).
fn receiver_name(node: &Node<'_>, ctx: &Context<'_>) -> String {
    let Some(call) = node.as_call_node() else {
        return String::from_utf8_lossy(ctx.text(node.span())).into_owned();
    };
    if let Some(recv) = call.receiver() {
        if !is_const_node(&recv) {
            return receiver_name(&recv, ctx);
        }
    }
    // `receiver.send_type?`: excludes safe navigation (`csend`) explicitly.
    if call.is_safe_navigation() {
        return String::from_utf8_lossy(ctx.text(node.span())).into_owned();
    }
    match call.receiver() {
        Some(recv) => {
            format!(
                "{}.{}",
                receiver_name(&recv, ctx),
                String::from_utf8_lossy(call.name().as_slice())
            )
        }
        None => String::from_utf8_lossy(call.name().as_slice()).into_owned(),
    }
}

/// `each_key`/`each_value` for a `keys`/`values` method name.
fn each_prefer(method_name: &[u8]) -> &'static str {
    match method_name {
        b"keys" => "each_key",
        b"values" => "each_value",
        _ => unreachable!("caller already matched keys/values"),
    }
}

/// Use `Hash#each_key` and `Hash#each_value`.
#[derive(Debug, Clone)]
pub struct HashEachMethods {
    /// `AllowedReceivers`: receiver names ([`receiver_name`]) never flagged.
    allowed_receivers: Vec<String>,
}

impl HashEachMethods {
    fn is_allowed_receiver(&self, node: &Node<'_>, ctx: &Context<'_>) -> bool {
        let name = receiver_name(node, ctx);
        self.allowed_receivers.contains(&name)
    }

    /// `register_kv_offense`: `call` is `_.keys.each`/`_.values.each` with a
    /// literal block; `inner` is the `keys`/`values` call.
    fn register_kv_offense(
        &self,
        call: &CallNode<'_>,
        inner: &CallNode<'_>,
        ctx: &mut Context<'_>,
    ) {
        let Some(parent_receiver) = inner.receiver() else { return };
        if self.is_allowed_receiver(&parent_receiver, ctx) {
            return;
        }
        let Some(inner_sel) = inner.message_loc() else { return };
        let Some(outer_sel) = call.message_loc() else { return };
        let report_span = Span::new(inner_sel.span().start, outer_sel.span().end);

        let target_span = ext::call_span_excluding_block(call);
        let current_span = Span::new(inner_sel.span().start, target_span.end);
        let current_text = String::from_utf8_lossy(ctx.text(current_span)).into_owned();
        let prefer = each_prefer(inner.name().as_slice());
        let message = format!("Use `{prefer}` instead of `{current_text}`.");

        let receiver_text = ctx.text(parent_receiver.span());
        let dot_text = call.call_operator_loc().map_or(&b"."[..], |l| ctx.text(l.span()));
        let mut new_source =
            Vec::with_capacity(receiver_text.len() + dot_text.len() + prefer.len());
        new_source.extend_from_slice(receiver_text);
        new_source.extend_from_slice(dot_text);
        new_source.extend_from_slice(prefer.as_bytes());

        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(target_span, new_source)],
        };
        ctx.report_with_fix(&Self::META, report_span, message, fix);
    }

    /// `register_kv_with_block_pass_offense`: `call` is
    /// `_.keys.each(&:sym)`/`_.values.each(&:sym)`; `inner` is the
    /// `keys`/`values` call.
    fn register_kv_with_block_pass_offense(
        &self,
        call: &CallNode<'_>,
        inner: &CallNode<'_>,
        ctx: &mut Context<'_>,
    ) {
        let Some(parent_receiver) = inner.receiver() else { return };
        if self.is_allowed_receiver(&parent_receiver, ctx) {
            return;
        }
        let Some(inner_sel) = inner.message_loc() else { return };
        let Some(outer_sel) = call.message_loc() else { return };
        let range = Span::new(inner_sel.span().start, outer_sel.span().end);
        let range_text = String::from_utf8_lossy(ctx.text(range)).into_owned();
        let prefer = each_prefer(inner.name().as_slice());
        let message = format!("Use `{prefer}` instead of `{range_text}`.");

        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(range, prefer.as_bytes().to_vec())],
        };
        ctx.report_with_fix(&Self::META, range, message, fix);
    }

    /// `check_unused_block_args`.
    fn check_unused_block_args(
        call: &CallNode<'_>,
        block: &BlockNode<'_>,
        key: &Node<'_>,
        value: &Node<'_>,
        ctx: &mut Context<'_>,
    ) {
        let Some(body) = block.body() else { return };
        let lvar_sources = collect_lvar_sources(&body);
        let value_unused = param_unused(value, &lvar_sources);
        let key_unused = param_unused(key, &lvar_sources);
        if value_unused && key_unused {
            return;
        }
        let Some(sel) = call.message_loc() else { return };
        let node_span = call.as_node().span();

        let (prefer, unused_span, unused_code_span) = if value_unused {
            (b"each_key".as_slice(), Span::new(key.span().end, value.span().end), value.span())
        } else if key_unused {
            (b"each_value".as_slice(), Span::new(key.span().start, value.span().start), key.span())
        } else {
            return;
        };
        let unused_code = String::from_utf8_lossy(ctx.text(unused_code_span)).into_owned();
        let prefer_str = std::str::from_utf8(prefer).expect("ascii literal");
        let message = format!(
            "Use `{prefer_str}` instead of `each` and remove the unused `{unused_code}` block \
             argument."
        );
        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(sel.span(), prefer.to_vec()), Edit::delete(unused_span)],
        };
        ctx.report_with_fix(&Self::META, node_span, message, fix);
    }
}

impl Rule for HashEachMethods {
    const META: RuleMeta = RuleMeta {
        name: "Style/HashEachMethods",
        department: Department::Style,
        summary: "Use Hash#each_key and Hash#each_value.",
        explanation: "\
Checks for uses of `each_key` and `each_value` `Hash` methods.

NOTE: If you have an array of two-element arrays, you can put parentheses
around the block arguments to indicate that you're not working with a hash,
and suppress offenses.

This cop is unsafe because it cannot be guaranteed that the receiver is a
`Hash`. The `AllowedReceivers` configuration can mitigate, but not fully
resolve, this safety issue.

```ruby
# bad
hash.keys.each { |k| p k }
hash.each { |k, unused_value| p k }

# good
hash.each_key { |k| p k }

# bad
hash.values.each { |v| p v }
hash.each { |unused_key, v| p v }

# good
hash.each_value { |v| p v }
```

With `AllowedReceivers: ['execute']`:

```ruby
# good
execute(sql).keys.each { |v| p v }
execute(sql).values.each { |v| p v }
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "AllowedReceivers",
            default: ConfigDefault::StrList(&[]),
            allowed: &[],
            doc: "Receiver names (`AllowedReceivers#receiver_name`) whose `keys.each`/\
                  `values.each`/`each` chain is never flagged.",
        }],
        blind_spots: "\
`hash_mutated?` (a receiver written to as `receiver[...] = ...` anywhere in the block body
suppresses the offense) is approximated by same-kind-and-same-source-text comparison rather than
upstream's full recursive AST equality; exact for the plain-identifier/bare-call receivers this
cop sees in practice. A block with two parameters directly on `_.keys.each`/`_.values.each`
(nonsensical Ruby, since `keys`/`values` yield one value) is not re-checked against
`each_arguments` after `kv_each` already matched, unlike upstream, which falls through when
`register_kv_offense` itself adds no offense (e.g. an allowed receiver).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allowed_receivers: options.str_list("AllowedReceivers") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        if call.name().as_slice() != b"each" {
            return;
        }

        if let Some(block_arg) = call.block().and_then(|b| b.as_block_argument_node()) {
            if block_arg.expression().is_some_and(|e| e.as_symbol_node().is_some()) {
                if let Some(inner) = match_kv_each_block_pass(&call) {
                    self.register_kv_with_block_pass_offense(&call, &inner, ctx);
                }
            }
            return;
        }

        let Some(block_node) = call.block().and_then(|b| b.as_block_node()) else { return };
        if !handleable(&call, ctx) {
            return;
        }

        if let Some(inner) = match_kv_each(&call) {
            self.register_kv_offense(&call, &inner, ctx);
            return;
        }

        if let Some((key, value)) = match_each_arguments(&call, &block_node) {
            Self::check_unused_block_args(&call, &block_node, &key, &value, ctx);
        }
    }
}
