//! `Style/HashTransformValues`, ported from RuboCop's
//! `lib/rubocop/cop/style/hash_transform_values.rb`. The matching and
//! autocorrect logic upstream lives in the shared `HashTransformMethod`
//! mixin (`lib/rubocop/cop/mixin/hash_transform_method.rb`, also used by
//! `Style/HashTransformKeys`); it is ported privately here rather than
//! shared, per the porting kit's file-isolation rule.
//!
//! Four shapes are recognised, each corresponding to one of the mixin's
//! `def_node_matcher`s, dispatched from a single `CallNode` visit (Prism has
//! no separate `on_block`/`on_send` split -- a block is just a field on its
//! call's node):
//!
//! - `each_with_object`: `_.each_with_object({}) { |(k, v), h| h[k] = ... }`.
//! - `to_h` with its own block: `_.to_h { |k, v| [k, ...] }` (gated on
//!   `target_ruby_version >= 2.6`, mirroring `return if target_ruby_version
//!   < 2.6` before `on_bad_to_h`; the whole cop's own `minimum_target_ruby_version
//!   2.4` becomes the `enabled` flag).
//! - `map`/`collect` immediately followed by `.to_h`: matched from the
//!   `to_h` call's *receiver*, regardless of ruby version and regardless of
//!   whether the `to_h` call itself also carries a trailing block (Prism's
//!   `CallNode::block` doesn't stop that from also being a `map.to_h`
//!   match, same as upstream's independent `on_send` firing on the inner
//!   `send` node).
//! - `Hash[_.map { ... }]`.
//!
//! In every case the "was a hash, not just key/value pairs" filtering is
//! the mixin's three post-match guards (`noop_transformation?`,
//! `transformation_uses_both_args?`, `use_transformed_argname?`), captured
//! here as [`Captures::should_report`]. The one pattern piece not
//! reproduced is the mixin's trailing "not a backreference to the memo
//! argument" guard on `on_bad_each_with_object`'s captured value expression
//! -- in whitequark that backreference can only ever equal a bare
//! method-argument symbol, which a full AST node is never `==` to, so the
//! guard always passes and never filters anything.
//!
//! Offense spans, and the "rename target"/block used to build the fix, use
//! [`ruby_ast::ext::call_span_excluding_block`] for the two `on_send`-shaped
//! matches (`Hash[]` and the `map.to_h` chain), since Prism's `CallNode`
//! span always extends through its own attached block -- relevant only for
//! `to_h`'s own optional trailing block, exercised by the
//! `correctly_autocorrects_map_to_h_with_block` fixture. The
//! `on_block`-shaped matches (`each_with_object`, `to_h { ... }`) report the
//! whole node, which already includes the block since it's the same node.
//!
//! The autocorrect mirrors `HashTransformMethod#execute_correction`:
//! rename the matched method to `transform_values`, replace the block's
//! parameter list with `|<val_argname>|`, replace the block's body with the
//! value expression's source (wrapping it in braces if it's a braceless
//! hash), and -- for the `Hash[...]` and `map.to_h` shapes -- delete the
//! wrapping `Hash[`/`]` or the trailing `.to_h` text.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, CallNode, ParametersNode};
use ruby_ast::{each_descendant, ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Prefer `transform_values` over `each_with_object`, `map`, or `to_h`.
#[derive(Debug, Clone)]
pub struct HashTransformValues {
    /// `minimum_target_ruby_version 2.4`: the whole cop is a no-op below it.
    enabled: bool,
    /// `return if target_ruby_version < 2.6` before `on_bad_to_h`: only the
    /// `to_h { ... }` block form needs this (`Enumerable#to_h` with a block
    /// is 2.6+); `each_with_object`, `map {...}.to_h`, and
    /// `Hash[_.map {...}]` are unguarded.
    check_to_h_block: bool,
}

impl Rule for HashTransformValues {
    const META: RuleMeta = RuleMeta {
        name: "Style/HashTransformValues",
        department: Department::Style,
        summary: "Checks for uses of `each_with_object`, `map`, and `to_h` that are actually just transforming the values of a hash, and prefers `transform_values` instead.",
        explanation: "\
Looks for uses of `each_with_object({})`, `map { ... }.to_h`, and \
`Hash[_.map { ... }]` that are actually just transforming the values of a \
hash, and tries to use a simpler & faster call to `transform_values` \
instead.

This cop is unsafe, as it can produce false positives if we are \
transforming an enumerable of key-value-like pairs that isn't actually a \
hash, e.g.: `[[k1, v1], [k2, v2], ...]`.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let target_ruby_version = options.target_ruby_version();
        Ok(Self {
            enabled: target_ruby_version >= 2.4,
            check_to_h_block: target_ruby_version >= 2.6,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();

        match name.as_slice() {
            b"each_with_object" => {
                if let Some((captures, block)) = match_each_with_object(&call) {
                    if captures.should_report() {
                        report_and_fix(
                            ctx,
                            call.as_node().span(),
                            "each_with_object",
                            &call,
                            &block,
                            &captures,
                            &[],
                        );
                    }
                }
            }
            b"to_h" => {
                if self.check_to_h_block && call.block().is_some() {
                    if let Some((captures, block)) = match_block_array_pair(&call) {
                        if captures.should_report() {
                            report_and_fix(
                                ctx,
                                call.as_node().span(),
                                "to_h {...}",
                                &call,
                                &block,
                                &captures,
                                &[],
                            );
                        }
                    }
                }
                if let Some(inner) = call.receiver().and_then(|r| r.as_call_node()) {
                    if let Some((captures, block)) = match_map_or_collect_block(&inner) {
                        if captures.should_report() {
                            let offense_span = ext::call_span_excluding_block(&call);
                            let strip = map_to_h_trailing_strip(&call, &inner, offense_span);
                            report_and_fix(
                                ctx,
                                offense_span,
                                "map {...}.to_h",
                                &inner,
                                &block,
                                &captures,
                                &strip,
                            );
                        }
                    }
                }
            }
            b"[]" => {
                if let Some((inner, block, captures)) = match_hash_brackets_map(&call) {
                    if captures.should_report() {
                        let outer_span = call.as_node().span();
                        let strip = vec![
                            Span::new(outer_span.start, outer_span.start + 5),
                            Span::new(outer_span.end - 1, outer_span.end),
                        ];
                        report_and_fix(
                            ctx,
                            ext::call_span_excluding_block(&call),
                            "Hash[_.map {...}]",
                            &inner,
                            &block,
                            &captures,
                            &strip,
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

/// Reports the offense and, if the port is confident nothing else could
/// have changed shape underneath it, its `Applicability::Unsafe` fix (the
/// whole cop is marked unsafe upstream: it can misfire on a non-`Hash`
/// enumerable of key/value-like pairs). `rename_target`/`block` are the
/// call whose method name becomes `transform_values` and whose block gets
/// new parameters/body; `strip` are extra byte ranges to delete (the
/// `Hash[`/`]` wrapper, or a `map {...}.to_h` chain's trailing `.to_h`).
fn report_and_fix(
    ctx: &mut Context<'_>,
    offense_span: Span,
    match_desc: &str,
    rename_target: &CallNode<'_>,
    block: &BlockNode<'_>,
    captures: &Captures<'_>,
    strip: &[Span],
) {
    let message = format!("Prefer `transform_values` over `{match_desc}`.");
    let body = body_replacement(ctx, &captures.val_expr);
    let mut edits = vec![
        Edit::replace(method_rename_span(rename_target), b"transform_values".to_vec()),
        Edit::replace(
            params_span(block),
            format!("|{}|", String::from_utf8_lossy(captures.val_argname)).into_bytes(),
        ),
        Edit::replace(body_span(block), body),
    ];
    edits.extend(strip.iter().copied().map(Edit::delete));
    ctx.report_with_fix(
        &HashTransformValues::META,
        offense_span,
        message,
        Fix { applicability: Applicability::Unsafe, edits },
    );
}

/// The range to replace with `transform_values`: the method name, extended
/// through the call's own closing paren if it has explicit arguments (only
/// `each_with_object({})` does).
fn method_rename_span(call: &CallNode<'_>) -> Span {
    let message = call.message_loc().expect("matched call always has a message location").span();
    match call.closing_loc() {
        Some(closing) => Span::new(message.start, closing.span().end),
        None => message,
    }
}

fn params_span(block: &BlockNode<'_>) -> Span {
    block
        .parameters()
        .expect("matched block always has parameters")
        .as_block_parameters_node()
        .expect("matched block's parameters are always a BlockParametersNode")
        .as_node()
        .span()
}

fn body_span(block: &BlockNode<'_>) -> Span {
    block.body().expect("matched block always has a body").span()
}

/// `Autocorrection#set_new_body_expression`: the value expression's source,
/// wrapped in braces if it's a braceless hash (`[k, value: v]`'s `value: v`
/// is a `KeywordHashNode` in Prism, never a braced `HashNode`).
fn body_replacement(ctx: &Context<'_>, val_expr: &Node<'_>) -> Vec<u8> {
    let src = ctx.text(val_expr.span());
    if val_expr.as_keyword_hash_node().is_some() {
        let mut out = Vec::with_capacity(src.len() + 4);
        out.extend_from_slice(b"{ ");
        out.extend_from_slice(src);
        out.extend_from_slice(b" }");
        out
    } else {
        src.to_vec()
    }
}

/// `Autocorrection.from_map_to_h`'s trailing-strip length: nothing, if the
/// `to_h` call carries its own trailing block (its own range already stops
/// short of that block); otherwise everything after the inner `map`/
/// `collect` call's own range (its trailing `.to_h`, possibly preceded by a
/// line break, as in `x.map {...}.\n  to_h`).
fn map_to_h_trailing_strip(
    to_h_call: &CallNode<'_>,
    inner: &CallNode<'_>,
    offense_span: Span,
) -> Vec<Span> {
    if to_h_call.block().is_some() {
        return Vec::new();
    }
    let inner_end = inner.as_node().span().end;
    if offense_span.end > inner_end {
        vec![Span::new(inner_end, offense_span.end)]
    } else {
        Vec::new()
    }
}

/// The mixin's `Captures` struct: the block's value-argument name, the key
/// expression referenced in the body (always a bare local variable read),
/// and the expression assigned/collected as the value.
struct Captures<'pr> {
    val_argname: &'pr [u8],
    key_expr: Node<'pr>,
    val_expr: Node<'pr>,
}

impl Captures<'_> {
    /// `HashTransformMethod#handle_possible_offense`'s three guards.
    fn should_report(&self) -> bool {
        // `noop_transformation?`: the value expression is just the bare
        // value argument, so the key is the only thing that would change.
        if self
            .val_expr
            .as_local_variable_read_node()
            .is_some_and(|l| l.name().as_slice() == self.val_argname)
        {
            return false;
        }
        // `transformation_uses_both_args?`: can't `transform_values` if the
        // value expression also depends on the key.
        if let Some(key_lvar) = self.key_expr.as_local_variable_read_node() {
            if contains_lvar_named(&self.val_expr, key_lvar.name().as_slice()) {
                return false;
            }
        }
        // `use_transformed_argname?`: the value expression must actually
        // reference the value argument, or the receiver likely isn't a hash.
        // `transforming_body_expr.splat_type?`
        contains_lvar_named(&self.val_expr, self.val_argname)
            && self.val_expr.as_splat_node().is_none()
    }
}

fn contains_lvar_named(node: &Node<'_>, name: &[u8]) -> bool {
    let mut found = false;
    each_descendant(node, &mut |d| {
        found |= d.as_local_variable_read_node().is_some_and(|l| l.name().as_slice() == name);
    });
    found
}

/// `#hash_receiver?`: a literal hash, or a call/block known to return a
/// hash (`to_h`, `merge`, `invert`, `group_by`, `each_with_object({})`,
/// etc.). Replaces the old array-receiver blacklist with upstream's
/// current whitelist -- the receiver of `each_with_object`, `map`/
/// `collect`, and `to_h` must match this before the cop fires at all.
fn is_hash_receiver(node: &Node<'_>) -> bool {
    if node.as_hash_node().is_some() {
        return true;
    }
    let Some(call) = node.as_call_node() else { return false };
    let name = call.name();
    let name = name.as_slice();
    let block = call.block().and_then(|b| b.as_block_node());
    // The pattern's `block` excludes whitequark's `numblock`/`itblock`.
    if block.as_ref().and_then(ruby_ast::node::BlockNode::parameters).is_some_and(|p| {
        p.as_it_parameters_node().is_some() || p.as_numbered_parameters_node().is_some()
    }) {
        return false;
    }
    match block {
        None => matches!(
            name,
            b"to_h"
                | b"to_hash"
                | b"merge"
                | b"merge!"
                | b"update"
                | b"invert"
                | b"except"
                | b"tally"
        ),
        Some(_) => match name {
            b"group_by" | b"to_h" | b"tally" | b"transform_keys" | b"transform_keys!"
            | b"transform_values" | b"transform_values!" => true,
            b"each_with_object" => {
                let args = call.arguments().map(|a| a.arguments());
                args.is_some_and(|args| {
                    args.len() == 1
                        && args.iter().next().is_some_and(|a| a.as_hash_node().is_some())
                })
            }
            _ => false,
        },
    }
}

/// The block's own parameters, required to be *exactly* two required
/// parameters (whether plain or the first one destructured) with no
/// optional/rest/post/keyword/block parameters, matching the mixin's
/// patterns having no trailing `...`.
fn required_params<'pr>(block: &BlockNode<'pr>) -> Option<ParametersNode<'pr>> {
    let params = block.parameters()?.as_block_parameters_node()?.parameters()?;
    if !params.optionals().is_empty()
        || params.rest().is_some()
        || !params.posts().is_empty()
        || !params.keywords().is_empty()
        || params.keyword_rest().is_some()
        || params.block().is_some()
    {
        return None;
    }
    Some(params)
}

/// `(args (arg _key)(arg $_))`: two plain required parameters, as in
/// `|k, v|`.
fn plain_two_param_names<'pr>(block: &BlockNode<'pr>) -> Option<(&'pr [u8], &'pr [u8])> {
    let reqs = required_params(block)?.requireds();
    if reqs.len() != 2 {
        return None;
    }
    let mut iter = reqs.iter();
    let key = iter.next()?.as_required_parameter_node()?;
    let val = iter.next()?.as_required_parameter_node()?;
    Some((key.name().as_slice(), val.name().as_slice()))
}

/// `(args (mlhs (arg _key)(arg $_)) (arg _memo))`: a destructured pair
/// followed by the memo, as in `|(k, v), h|`.
fn mlhs_two_and_memo_names<'pr>(
    block: &BlockNode<'pr>,
) -> Option<(&'pr [u8], &'pr [u8], &'pr [u8])> {
    let reqs = required_params(block)?.requireds();
    if reqs.len() != 2 {
        return None;
    }
    let mut iter = reqs.iter();
    let mlhs = iter.next()?.as_multi_target_node()?;
    let memo = iter.next()?.as_required_parameter_node()?;
    if mlhs.rest().is_some() || !mlhs.rights().is_empty() {
        return None;
    }
    let lefts = mlhs.lefts();
    if lefts.len() != 2 {
        return None;
    }
    let mut left_iter = lefts.iter();
    let key = left_iter.next()?.as_required_parameter_node()?;
    let val = left_iter.next()?.as_required_parameter_node()?;
    Some((key.name().as_slice(), val.name().as_slice(), memo.name().as_slice()))
}

/// The `each_with_object` block's single-statement body:
/// `(call (lvar _memo) :[]= $(lvar _key) $_)`, i.e. `h[k] = <val_expr>`.
fn each_with_object_body<'pr>(
    block: &BlockNode<'pr>,
    key_name: &[u8],
    memo_name: &[u8],
) -> Option<(Node<'pr>, Node<'pr>)> {
    let body = block.body()?.as_statements_node()?.body();
    if body.len() != 1 {
        return None;
    }
    let call = body.iter().next()?.as_call_node()?;
    if call.name().as_slice() != b"[]=" {
        return None;
    }
    if call.receiver()?.as_local_variable_read_node()?.name().as_slice() != memo_name {
        return None;
    }
    let args = call.arguments()?.arguments();
    if args.len() != 2 {
        return None;
    }
    let mut iter = args.iter();
    let key_expr = iter.next()?;
    let val_expr = iter.next()?;
    if key_expr.as_local_variable_read_node()?.name().as_slice() != key_name {
        return None;
    }
    Some((key_expr, val_expr))
}

/// `on_bad_each_with_object`: `_.each_with_object({}) { |(k, v), h| h[k] = ... }`.
fn match_each_with_object<'pr>(call: &CallNode<'pr>) -> Option<(Captures<'pr>, BlockNode<'pr>)> {
    if !is_hash_receiver(&call.receiver()?) {
        return None;
    }
    let args = call.arguments()?.arguments();
    if args.len() != 1 {
        return None;
    }
    let hash = args.iter().next()?.as_hash_node()?;
    if !hash.elements().is_empty() {
        return None;
    }
    let block = call.block()?.as_block_node()?;
    let (key_name, val_name, memo_name) = mlhs_two_and_memo_names(&block)?;
    let (key_expr, val_expr) = each_with_object_body(&block, key_name, memo_name)?;
    Some((Captures { val_argname: val_name, key_expr, val_expr }, block))
}

/// The shared shape of `on_bad_map_to_h`/`on_bad_hash_brackets_map`'s inner
/// block and `on_bad_to_h`'s own block: `(args (arg _key)(arg $_)) (array
/// $(lvar _key) $_)`, i.e. `{ |k, v| [k, <val_expr>] }`.
fn match_block_array_pair<'pr>(call: &CallNode<'pr>) -> Option<(Captures<'pr>, BlockNode<'pr>)> {
    if !is_hash_receiver(&call.receiver()?) {
        return None;
    }
    let block = call.block()?.as_block_node()?;
    let (key_name, val_name) = plain_two_param_names(&block)?;
    let body = block.body()?.as_statements_node()?.body();
    if body.len() != 1 {
        return None;
    }
    let array = body.iter().next()?.as_array_node()?;
    let elements = array.elements();
    if elements.len() != 2 {
        return None;
    }
    let mut iter = elements.iter();
    let key_expr = iter.next()?;
    let val_expr = iter.next()?;
    if key_expr.as_local_variable_read_node()?.name().as_slice() != key_name {
        return None;
    }
    Some((Captures { val_argname: val_name, key_expr, val_expr }, block))
}

fn match_map_or_collect_block<'pr>(
    call: &CallNode<'pr>,
) -> Option<(Captures<'pr>, BlockNode<'pr>)> {
    if !matches!(call.name().as_slice(), b"map" | b"collect") {
        return None;
    }
    match_block_array_pair(call)
}

/// `on_bad_hash_brackets_map`: `Hash[_.map { |k, v| [k, ...] }]`.
fn match_hash_brackets_map<'pr>(
    call: &CallNode<'pr>,
) -> Option<(CallNode<'pr>, BlockNode<'pr>, Captures<'pr>)> {
    let receiver = call.receiver()?;
    if receiver.as_constant_read_node()?.name().as_slice() != b"Hash" {
        return None;
    }
    let args = call.arguments()?.arguments();
    if args.len() != 1 {
        return None;
    }
    let inner = args.iter().next()?.as_call_node()?;
    let (captures, block) = match_map_or_collect_block(&inner)?;
    Some((inner, block, captures))
}
