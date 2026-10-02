//! `Lint/RedundantTypeConversion`, ported from RuboCop's
//! `lib/rubocop/cop/lint/redundant_type_conversion.rb`.
//!
//! Upstream matches whitequark node *types*: a bare `send` node for most
//! constructor patterns, versus a `block` node wrapping a bare
//! `Hash.new`/`Hash.[]` send for the one pattern that allows a literal
//! block. Prism never splits those into two node types -- a call's literal
//! block (if any) always lives in `CallNode::block()` regardless of
//! receiver or argument shape -- so [`has_literal_block`] stands in for
//! "this would have been a `:block`-type node upstream", gating every
//! constructor matcher except the dedicated `Hash.new { ... }` branch in
//! [`hash_constructor`].
//!
//! Similarly, whitequark counts a `&block`/`&:sym` pass (`block_pass`) as an
//! ordinary trailing `send` argument, so `node.arguments.any?` already
//! catches `to_set(&:foo)` before `hash_or_set_with_block?` ever inspects
//! `last_argument`. Prism keeps that pass out of `arguments()` entirely, in
//! `CallNode::block()` as a `BlockArgumentNode` instead, so
//! [`hash_or_set_with_block`] folds both of upstream's checks (a literal
//! block *and* a block-pass argument) into one: any `block()` at all on a
//! `to_h`/`to_set` call.
//!
//! `((foo.to_s)).to_s` shows parentheses are `ParenthesesNode` wrapping a
//! `StatementsNode`, not whitequark's single `begin` node with `children`;
//! [`find_receiver`] walks that one-statement-at-a-time shape instead.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const CONVERSION_METHODS: &[&[u8]] =
    &[b"to_s", b"to_sym", b"to_i", b"to_f", b"to_r", b"to_c", b"to_a", b"to_h", b"to_set"];

/// `RESTRICT_ON_SEND = CONVERSION_METHODS + [:to_d]`.
fn is_restricted_method(name: &[u8]) -> bool {
    name == b"to_d" || CONVERSION_METHODS.contains(&name)
}

/// Checks for redundantly converting a literal to the same type.
#[derive(Debug, Clone, Default)]
pub struct RedundantTypeConversion {
    /// For each currently-open `BlockNode`, whether its body is exactly one
    /// statement (no synthetic multi-statement wrapper). Whitequark's
    /// `node.parent&.any_block_type?` check in `hash_or_set_with_block?` is
    /// true both when `node` itself owns a literal block (handled
    /// separately by [`hash_or_set_with_block`]'s `call.block()` check) and
    /// -- a byproduct of whitequark always wrapping a `:block` node's own
    /// call/body as direct children -- when `node` merely happens to be the
    /// sole statement of some unrelated enclosing block. Prism's
    /// `BlockNode::body()` is a `StatementsNode` sibling field rather than
    /// an ancestor-defining wrapper, so this stack reconstructs that
    /// "sole statement" shape from the real tree instead.
    single_statement_block: Vec<bool>,
}

impl Rule for RedundantTypeConversion {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RedundantTypeConversion",
        department: Department::Lint,
        summary: "Checks for redundantly converting a literal to the same type.",
        explanation: "Checks for redundant uses of `to_s`, `to_sym`, `to_i`, `to_f`, `to_d`, \
            `to_r`, `to_c`, `to_a`, `to_h`, and `to_set`.\n\nWhen one of these methods is called \
            on an object of the same type, that object is returned, making the call \
            unnecessary.",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::BlockNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(block) = node.as_block_node() {
            let is_single = block
                .body()
                .and_then(|b| b.as_statements_node())
                .is_some_and(|s| s.body().len() == 1);
            self.single_statement_block.push(is_single);
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let method = call.name().as_slice();
        if !is_restricted_method(method) {
            return;
        }
        if has_arguments(&call)
            || hash_or_set_with_block(&call, method, ctx, &self.single_statement_block)
        {
            return;
        }
        let Some(receiver) = find_receiver(call.receiver()) else { return };
        let redundant = literal_receiver(method, &receiver)
            || constructor(method, &receiver)
            || chained_conversion(method, &receiver)
            || chained_to_typed_method(method, &receiver);
        if !redundant {
            return;
        }

        let Some(message_loc) = call.message_loc() else { return };
        let span = message_loc.span();
        let method_str = String::from_utf8_lossy(method);
        let message = format!("Redundant `{method_str}` detected.");

        let Some(dot) = call.call_operator_loc() else {
            ctx.report(&Self::META, span, message);
            return;
        };
        let end = call.closing_loc().map_or(span.end, |c| c.span().end);
        let remove_span = Span::new(dot.span().start, end);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(remove_span)] },
        );
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.as_block_node().is_some() {
            self.single_statement_block.pop();
        }
    }
}

/// `node.arguments.any?`.
fn has_arguments(call: &CallNode<'_>) -> bool {
    call.arguments().is_some_and(|a| !a.arguments().is_empty())
}

/// `hash_or_set_with_block?`: upstream also inspects `last_argument` for a
/// whitequark `block_pass`, which Prism keeps out of `arguments()` and in
/// `block()` instead (see module docs), so any attached block -- literal or
/// `&block`/`&:sym` pass -- disqualifies a bare `to_h`/`to_set` call. Also
/// reproduces `node.parent&.any_block_type?`'s accidental second meaning:
/// whitequark wraps *any* block's own call/body as direct children of its
/// `:block` node, so this is also true when `node` is merely the sole
/// statement of some unrelated enclosing block (see the struct doc).
fn hash_or_set_with_block(
    call: &CallNode<'_>,
    method: &[u8],
    ctx: &Context<'_>,
    single_statement_block: &[bool],
) -> bool {
    if method != b"to_h" && method != b"to_set" {
        return false;
    }
    if call.block().is_some() {
        return true;
    }
    let ancestors = ctx.ancestors();
    let len = ancestors.len();
    len >= 2
        && ancestors[len - 1].kind == NodeKind::StatementsNode
        && ancestors[len - 2].kind == NodeKind::BlockNode
        && single_statement_block.last().copied().unwrap_or(false)
}

/// `find_receiver`: unwraps a parenthesized single-statement receiver,
/// repeatedly. Prism's `ParenthesesNode` always wraps a `StatementsNode`
/// (even for one statement), unlike whitequark's single-child `begin`.
fn find_receiver(receiver: Option<Node<'_>>) -> Option<Node<'_>> {
    let mut receiver = receiver?;
    while let Some(next) = unwrap_single_statement_parens(&receiver) {
        receiver = next;
    }
    Some(receiver)
}

/// Unwraps one layer of `(stmt)` around `node`, if `node` is a
/// `ParenthesesNode` whose body is a single-statement `StatementsNode`.
fn unwrap_single_statement_parens<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let parens = node.as_parentheses_node()?;
    let statements = parens.body()?.as_statements_node()?;
    let stmts = statements.body();
    if stmts.len() != 1 {
        return None;
    }
    stmts.first()
}

/// `LITERAL_NODE_TYPES` + `literal_receiver?`.
fn literal_receiver(method: &[u8], receiver: &Node<'_>) -> bool {
    matches!(
        (method, receiver.kind()),
        (b"to_s", NodeKind::StringNode | NodeKind::InterpolatedStringNode)
            | (b"to_sym", NodeKind::SymbolNode | NodeKind::InterpolatedSymbolNode)
            | (b"to_i", NodeKind::IntegerNode)
            | (b"to_f", NodeKind::FloatNode)
            | (b"to_r", NodeKind::RationalNode)
            | (b"to_c", NodeKind::ImaginaryNode)
            | (b"to_a", NodeKind::ArrayNode)
            | (b"to_h", NodeKind::HashNode)
    )
}

/// `CONSTRUCTOR_MAPPING` + `constructor?`: dispatches to the matcher for
/// `method`, then rejects a type constructor that suppresses exceptions
/// (`exception: false`), which could return `nil`.
fn constructor(method: &[u8], receiver: &Node<'_>) -> bool {
    let Some(call) = receiver.as_call_node() else { return false };
    let matched = match method {
        b"to_s" => string_constructor(&call),
        b"to_i" => type_constructor(&call, b"Integer"),
        b"to_f" => type_constructor(&call, b"Float"),
        b"to_d" => type_constructor(&call, b"BigDecimal"),
        b"to_r" => type_constructor(&call, b"Rational"),
        b"to_c" => type_constructor(&call, b"Complex"),
        b"to_a" => array_constructor(&call),
        b"to_h" => hash_constructor(&call),
        b"to_set" => set_constructor(&call),
        _ => false,
    };
    matched && !constructor_suppresses_exceptions(&call)
}

/// Whether `call`'s attached block, if any, is a literal block (`{ }`/
/// `do...end`) rather than a `&block`/`&:sym` pass (`BlockArgumentNode`).
/// None of the constructor patterns below have a whitequark `block`-node
/// alternative except `Hash.new { ... }` (handled separately in
/// [`hash_constructor`]), so a literal block here means "this call's node
/// type upstream was `:block`, not `:send`" -- never a match.
fn has_literal_block(call: &CallNode<'_>) -> bool {
    call.block().is_some_and(|b| b.as_block_node().is_some())
}

/// Whether `receiver` is a bare or top-level (`::Foo`) reference to
/// constant `name`; whitequark's `{cbase nil?}`.
fn const_named(node: Option<Node<'_>>, name: &[u8]) -> bool {
    let Some(node) = node else { return false };
    if let Some(c) = node.as_constant_read_node() {
        return c.name().as_slice() == name;
    }
    if let Some(c) = node.as_constant_path_node() {
        return c.parent().is_none() && c.name().is_some_and(|n| n.as_slice() == name);
    }
    false
}

/// `type_constructor?(node, type_symbol)`: `Type(...)`, called with no
/// receiver or on `Kernel`.
fn type_constructor(call: &CallNode<'_>, type_name: &[u8]) -> bool {
    if has_literal_block(call) {
        return false;
    }
    (call.receiver().is_none() || const_named(call.receiver(), b"Kernel"))
        && call.name().as_slice() == type_name
}

/// `string_constructor?`.
fn string_constructor(call: &CallNode<'_>) -> bool {
    if has_literal_block(call) {
        return false;
    }
    (const_named(call.receiver(), b"String") && call.name().as_slice() == b"new")
        || type_constructor(call, b"String")
}

/// `array_constructor?`.
fn array_constructor(call: &CallNode<'_>) -> bool {
    if has_literal_block(call) {
        return false;
    }
    (const_named(call.receiver(), b"Array") && matches!(call.name().as_slice(), b"new" | b"[]"))
        || type_constructor(call, b"Array")
}

/// `set_constructor?`.
fn set_constructor(call: &CallNode<'_>) -> bool {
    if has_literal_block(call) {
        return false;
    }
    const_named(call.receiver(), b"Set") && matches!(call.name().as_slice(), b"new" | b"[]")
}

/// `hash_constructor?`: `Hash.new`/`Hash.[]`, a bare `Hash.new { ... }`
/// block (Prism attaches the literal block to the same `Hash.new` call
/// rather than wrapping it in a separate `:block` node), or the `Kernel`
/// method-style `Hash(...)`.
fn hash_constructor(call: &CallNode<'_>) -> bool {
    let is_new_or_bracket =
        const_named(call.receiver(), b"Hash") && matches!(call.name().as_slice(), b"new" | b"[]");
    if is_new_or_bracket {
        return match call.block() {
            Some(b) if b.as_block_node().is_some() => call.arguments().is_none(),
            _ => true,
        };
    }
    (call.receiver().is_none() || const_named(call.receiver(), b"Kernel"))
        && call.name().as_slice() == b"Hash"
        && !has_literal_block(call)
}

/// `constructor_suppresses_exceptions?`: any argument is an
/// `exception: false` keyword pair.
fn constructor_suppresses_exceptions(call: &CallNode<'_>) -> bool {
    let Some(args) = call.arguments() else { return false };
    args.arguments().iter().any(|arg| {
        let Some(kw) = arg.as_keyword_hash_node() else { return false };
        kw.elements().iter().any(|el| {
            let Some(assoc) = el.as_assoc_node() else { return false };
            let Some(sym) = assoc.key().as_symbol_node() else { return false };
            sym.unescaped() == b"exception" && matches!(assoc.value(), Node::FalseNode { .. })
        })
    })
}

/// `chained_conversion?`: the receiver is itself a call to the same
/// conversion method.
fn chained_conversion(method: &[u8], receiver: &Node<'_>) -> bool {
    receiver.as_call_node().is_some_and(|c| c.name().as_slice() == method)
}

/// `TYPED_METHODS` + `chained_to_typed_method?`: `foo.inspect.to_s` and
/// `foo.to_json.to_s`.
fn chained_to_typed_method(method: &[u8], receiver: &Node<'_>) -> bool {
    if method != b"to_s" {
        return false;
    }
    receiver.as_call_node().is_some_and(|c| matches!(c.name().as_slice(), b"inspect" | b"to_json"))
}
