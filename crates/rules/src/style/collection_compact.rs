//! `Style/CollectionCompact`, ported from RuboCop's
//! `lib/rubocop/cop/style/collection_compact.rb`.
//!
//! Upstream's node patterns use the meta type `call`, which matches both
//! `send` and `csend` (safe navigation); this port never checks
//! `CallNode::is_safe_navigation`, which reproduces that for both the
//! flagged call itself (`on_csend` is aliased to `on_send`) and the inner
//! `nil?`/`!` calls inside a block body.
//!
//! `reject_method?`/`select_method?` capture `$(args ...)` (the block's
//! declared parameters) and compare `args.last.source` against the matched
//! `lvar`'s source; [`block_last_param`] reconstructs "last declared
//! parameter" from a `ParametersNode`'s separate required/optional/rest/
//! post/keyword/keyword-rest/block-arg lists by picking the one that starts
//! latest in the source, since Prism groups them by kind rather than
//! textual order.
//!
//! `numblock`/`itblock` bodies read their implicit parameter back as
//! `LocalVariableReadNode` named `_1`/`ItLocalVariableReadNode`
//! respectively (see `docs/porting/KIT.md`); upstream's patterns for them
//! hardcode `_1`/`it` (not `_2` and up), so this port does too.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::is_bare_or_toplevel_const;
use ruby_ast::node::{BlockNode, CallNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use `{Array,Hash}#{compact,compact!}` instead of custom logic to reject nils.
#[derive(Debug, Clone)]
pub struct CollectionCompact {
    /// `AllowedReceivers`: receiver names ([`receiver_name`]) never flagged.
    allowed_receivers: Vec<String>,
    /// RuboCop's `target_ruby_version`, read once at configure time.
    target_ruby_version: f32,
}

impl Rule for CollectionCompact {
    const META: RuleMeta = RuleMeta {
        name: "Style/CollectionCompact",
        department: Department::Style,
        summary: "Use `{Array,Hash}#{compact,compact!}` instead of custom logic to reject nils.",
        explanation: "Checks for places where custom logic on rejection nils from arrays \
and hashes can be replaced with `{Array,Hash}#{compact,compact!}`.\n\n\
It is unsafe by default because false positives may occur in the `nil` \
check of block arguments to the receiver object. Additionally, we can't \
know the type of the receiver object for sure, which may result in false \
positives as well.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[linter::ConfigOption {
            name: "AllowedReceivers",
            default: linter::ConfigDefault::StrList(&[]),
            allowed: &[],
            doc: "Allowed receiver names (`AllowedReceivers#receiver_name`) that are never \
flagged.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allowed_receivers: options.str_list("AllowedReceivers"),
            target_ruby_version: options.target_ruby_version(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        let name = call.name();
        let name = name.as_slice();
        if !matches!(
            name,
            b"reject" | b"reject!" | b"select" | b"select!" | b"filter" | b"filter!" | b"grep_v"
        ) {
            return;
        }
        if self.target_ruby_version < 2.6 && matches!(name, b"filter" | b"filter!") {
            return;
        }
        let Some(range) = offense_range(&call, node.span(), ctx) else { return };
        if let Some(receiver) = call.receiver() {
            if self.allowed_receivers.contains(&receiver_name(&receiver, ctx)) {
                return;
            }
        }
        if self.target_ruby_version <= 3.0 && is_to_enum_method(&call) {
            return;
        }

        let good = if name.ends_with(b"!") { "compact!" } else { "compact" };
        let bad = String::from_utf8_lossy(ctx.text(range)).into_owned();
        let message = format!("Use `{good}` instead of `{bad}`.");
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, good.as_bytes().to_vec())],
            },
        );
    }
}

/// `offense_range`: either `range(node, node)` for
/// `reject_method_with_block_pass?`/`grep_v_with_nil?`, or
/// `range(node, block_node)` when the call carries an attached block whose
/// body matches [`match_block_method`].
fn offense_range(call: &CallNode<'_>, call_span: Span, ctx: &Context<'_>) -> Option<Span> {
    if is_reject_with_block_pass(call) || is_grep_v_with_nil(call) {
        let message_loc = call.message_loc()?;
        return Some(Span::new(message_loc.span().start, call_span.end));
    }
    let block = call.block()?.as_block_node()?;
    if !match_block_method(call, &block, ctx) {
        return None;
    }
    let message_loc = call.message_loc()?;
    Some(Span::new(message_loc.span().start, block.location().span().end))
}

/// `reject_method_with_block_pass?`: `(call !nil? {:reject :reject!}
/// (block_pass (sym :nil?)))`.
fn is_reject_with_block_pass(call: &CallNode<'_>) -> bool {
    if !matches!(call.name().as_slice(), b"reject" | b"reject!") {
        return false;
    }
    if call.receiver().is_none() {
        return false;
    }
    let Some(block) = call.block() else { return false };
    let Some(block_arg) = block.as_block_argument_node() else { return false };
    let Some(expr) = block_arg.expression() else { return false };
    let Some(sym) = expr.as_symbol_node() else { return false };
    sym.unescaped() == b"nil?"
}

/// `grep_v_with_nil?`: `(call _ :grep_v {(nil) (const {nil? cbase}
/// :NilClass)})`.
fn is_grep_v_with_nil(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() != b"grep_v" {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let args: Vec<Node<'_>> = args.arguments().iter().collect();
    let [arg] = args.as_slice() else { return false };
    if arg.as_nil_node().is_some() {
        return true;
    }
    is_nil_class_const(arg)
}

/// `(const {nil? cbase} :NilClass)`.
fn is_nil_class_const(node: &Node<'_>) -> bool {
    if !is_bare_or_toplevel_const(node) {
        return false;
    }
    match node.kind() {
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().expect("kind matched").name().as_slice() == b"NilClass"
        }
        NodeKind::ConstantPathNode => node
            .as_constant_path_node()
            .expect("kind matched")
            .name()
            .is_some_and(|n| n.as_slice() == b"NilClass"),
        _ => false,
    }
}

/// `match_block_method?`: the flagged call's own receiver must be present
/// (`!nil?` in both `reject_method?`/`select_method?`), then dispatches on
/// the block's parameter shape -- a plain block uses
/// [`is_reject_block`]/[`is_select_block`] (comparing the last declared
/// parameter against the referenced variable); `numblock`/`itblock` use the
/// hardcoded-parameter variants.
fn match_block_method(call: &CallNode<'_>, block: &BlockNode<'_>, ctx: &Context<'_>) -> bool {
    if call.receiver().is_none() {
        return false;
    }
    let is_reject_call = matches!(call.name().as_slice(), b"reject" | b"reject!");
    let is_select_call =
        matches!(call.name().as_slice(), b"select" | b"select!" | b"filter" | b"filter!");
    match block.parameters().map(|p| p.kind()) {
        None | Some(NodeKind::BlockParametersNode) => {
            (is_reject_call && is_reject_block(block, ctx))
                || (is_select_call && is_select_block(block, ctx))
        }
        Some(NodeKind::NumberedParametersNode) => {
            (is_reject_call && is_reject_numbered(block, b"_1"))
                || (is_select_call && is_select_numbered(block, b"_1"))
        }
        Some(NodeKind::ItParametersNode) => {
            (is_reject_call && is_reject_it(block)) || (is_select_call && is_select_it(block))
        }
        _ => false,
    }
}

/// The block's single body statement, or `None` for an empty/multi-statement
/// body.
fn single_statement<'pr>(block: &BlockNode<'pr>) -> Option<Node<'pr>> {
    let body = block.body()?.as_statements_node()?;
    let items = body.body();
    (items.len() == 1).then(|| items.first()).flatten()
}

/// The last declared block parameter by source position: `ParametersNode`
/// groups parameters by kind rather than textual order, so every list is
/// collected and the one that starts latest is picked.
fn block_last_param<'pr>(block: &BlockNode<'pr>) -> Option<Node<'pr>> {
    let params = block.parameters()?.as_block_parameters_node()?;
    let inner = params.parameters()?;
    let mut all: Vec<Node<'pr>> = Vec::new();
    all.extend(inner.requireds().iter());
    all.extend(inner.optionals().iter());
    if let Some(rest) = inner.rest() {
        all.push(rest);
    }
    all.extend(inner.posts().iter());
    all.extend(inner.keywords().iter());
    if let Some(keyword_rest) = inner.keyword_rest() {
        all.push(keyword_rest);
    }
    if let Some(block_arg) = inner.block() {
        all.push(block_arg.as_node());
    }
    all.into_iter().max_by_key(|n| n.span().start)
}

/// `reject_method?`: single-statement body `(call $(lvar _) :nil?)`, whose
/// receiver's source matches the last declared parameter's source.
fn is_reject_block(block: &BlockNode<'_>, ctx: &Context<'_>) -> bool {
    let Some(stmt) = single_statement(block) else { return false };
    let Some(call) = stmt.as_call_node() else { return false };
    if call.name().as_slice() != b"nil?" || call.arguments().is_some() {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    let Some(last_param) = block_last_param(block) else { return false };
    ctx.text(last_param.span()) == ctx.text(receiver.span())
}

/// `select_method?`: single-statement body `(call (call $(lvar _) :nil?)
/// :!)`.
fn is_select_block(block: &BlockNode<'_>, ctx: &Context<'_>) -> bool {
    let Some(stmt) = single_statement(block) else { return false };
    let Some(bang) = stmt.as_call_node() else { return false };
    if bang.name().as_slice() != b"!" || bang.arguments().is_some() {
        return false;
    }
    let Some(inner) = bang.receiver() else { return false };
    let Some(inner) = inner.as_call_node() else { return false };
    if inner.name().as_slice() != b"nil?" || inner.arguments().is_some() {
        return false;
    }
    let Some(receiver) = inner.receiver() else { return false };
    let Some(last_param) = block_last_param(block) else { return false };
    ctx.text(last_param.span()) == ctx.text(receiver.span())
}

/// `reject_method_for_numblock_or_itblock?`'s `numblock` arm: `(call
/// (lvar :_1) :nil?)`.
fn is_reject_numbered(block: &BlockNode<'_>, var: &[u8]) -> bool {
    let Some(stmt) = single_statement(block) else { return false };
    let Some(call) = stmt.as_call_node() else { return false };
    if call.name().as_slice() != b"nil?" || call.arguments().is_some() {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    receiver.as_local_variable_read_node().is_some_and(|lv| lv.name().as_slice() == var)
}

/// `select_method_for_numblock_or_itblock?`'s `numblock` arm: `(call (call
/// (lvar :_1) :nil?) :!)`.
fn is_select_numbered(block: &BlockNode<'_>, var: &[u8]) -> bool {
    let Some(stmt) = single_statement(block) else { return false };
    let Some(bang) = stmt.as_call_node() else { return false };
    if bang.name().as_slice() != b"!" || bang.arguments().is_some() {
        return false;
    }
    let Some(inner) = bang.receiver() else { return false };
    let Some(inner) = inner.as_call_node() else { return false };
    if inner.name().as_slice() != b"nil?" || inner.arguments().is_some() {
        return false;
    }
    let Some(receiver) = inner.receiver() else { return false };
    receiver.as_local_variable_read_node().is_some_and(|lv| lv.name().as_slice() == var)
}

/// `itblock` arm: `(call (lvar :it) :nil?)`, Prism's `it` reads back as
/// `ItLocalVariableReadNode`.
fn is_reject_it(block: &BlockNode<'_>) -> bool {
    let Some(stmt) = single_statement(block) else { return false };
    let Some(call) = stmt.as_call_node() else { return false };
    if call.name().as_slice() != b"nil?" || call.arguments().is_some() {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    receiver.as_it_local_variable_read_node().is_some()
}

/// `itblock` arm: `(call (call (lvar :it) :nil?) :!)`.
fn is_select_it(block: &BlockNode<'_>) -> bool {
    let Some(stmt) = single_statement(block) else { return false };
    let Some(bang) = stmt.as_call_node() else { return false };
    if bang.name().as_slice() != b"!" || bang.arguments().is_some() {
        return false;
    }
    let Some(inner) = bang.receiver() else { return false };
    let Some(inner) = inner.as_call_node() else { return false };
    if inner.name().as_slice() != b"nil?" || inner.arguments().is_some() {
        return false;
    }
    let Some(receiver) = inner.receiver() else { return false };
    receiver.as_it_local_variable_read_node().is_some()
}

/// `to_enum_method?`: the flagged call's receiver is itself a bare
/// `to_enum`/`lazy` call.
fn is_to_enum_method(call: &CallNode<'_>) -> bool {
    let Some(receiver) = call.receiver() else { return false };
    let Some(receiver) = receiver.as_call_node() else { return false };
    matches!(receiver.name().as_slice(), b"to_enum" | b"lazy")
}

/// `AllowedReceivers#receiver_name`. Only a `CallNode` ever has a receiver
/// at all; every other node kind falls straight to the `else` branch
/// (`receiver.source`).
fn receiver_name(node: &Node<'_>, ctx: &Context<'_>) -> String {
    let Some(call) = node.as_call_node() else {
        return String::from_utf8_lossy(ctx.text(node.span())).into_owned();
    };
    if let Some(recv) = call.receiver() {
        if !matches!(recv.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode) {
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
