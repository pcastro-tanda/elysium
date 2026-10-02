//! `Style/OperatorMethodCall`, ported from RuboCop's
//! `lib/rubocop/cop/style/operator_method_call.rb`.
//!
//! whitequark's `node.arguments` (used by `node.arguments.one?` and
//! `node.first_argument`) counts a trailing `&block`/`&:sym` pass as part of
//! a `send` node's own argument list; Prism splits it out into `block()` as
//! a [`ruby_ast::BlockArgumentNode`] instead of `arguments()`. The "one
//! argument" count and "first argument" lookup below fold that back in.
//!
//! whitequark's distinct `:forwarded_restarg`/`:forwarded_kwrestarg` node
//! types (a bare anonymous `*`/`**` forward) collapse onto the same Prism
//! node kinds as a named splat/double-splat (`SplatNode`/`AssocSplatNode`,
//! with `expression`/`value` `None` instead of `Some`); both forms are
//! excluded identically here, so the distinction is immaterial.
//!
//! `argument.parent.parent&.send_type?` and `node.parent&.call_type?` (is
//! some node's *parent* a plain method call) have no parent-node handle in
//! this engine's `Context`, only [`linter::Context::parent`]'s `NodeInfo`
//! (kind + span); `node.parent.first_argument == node` (is `node` used as
//! the parent's receiver rather than passed as one of its arguments) is
//! approximated by comparing byte offsets: a call used as another call's
//! *receiver* always starts at the same offset as that outer call (the
//! receiver is always the leftmost token), so an unequal start offset means
//! `node` was passed as an argument instead.
//!
//! Upstream registers only `on_send`, never `on_csend` -- a safe-navigation
//! operator call (`foo&.> bar`) is categorically never visited at all, not
//! merely excluded by some later guard. Prism gives both forms the same
//! [`NodeKind::CallNode`], distinguished only by `is_safe_navigation()`, so
//! that check stands in for the missing `on_csend` registration.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

const MSG: &str = "Redundant dot detected.";

const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"!", b"!=", b"!~",
];

/// Checks for redundant dot before operator method call.
#[derive(Debug, Clone)]
pub struct OperatorMethodCall;

impl Rule for OperatorMethodCall {
    const META: RuleMeta = RuleMeta {
        name: "Style/OperatorMethodCall",
        department: Department::Style,
        summary: "Checks for redundant dot before operator method call.",
        explanation: "\
The target operator methods are `|`, `^`, `&`, `<=>`, `==`, `===`, `=~`,
`>`, `>=`, `<`, `<=`, `<<`, `>>`, `+`, `-`, `*`, `/`, `%`, `**`, `~`, `!`,
`!=`, and `!~`.

```ruby
# bad
foo.+ bar
foo.& bar

# good
foo + bar
foo & bar
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let name = name.as_slice();
        if !OPERATOR_METHODS.contains(&name) {
            return;
        }
        let Some(dot) = call.call_operator_loc() else { return };
        let dot = dot.span();
        if call.is_safe_navigation() {
            // Upstream only ever registers `on_send`, never `on_csend`, so
            // a safe-navigation call (`foo&.> bar`) is never visited at
            // all -- not merely excluded after the fact.
            return;
        }
        if unary_method_no_operator(&call, ctx) {
            return;
        }
        if call.receiver().is_some_and(|r| {
            r.as_constant_read_node().is_some() || r.as_constant_path_node().is_some()
        }) {
            return;
        }
        let Some(rhs) = single_argument(&call) else { return };
        if method_call_with_parenthesized_arg(ctx, &call, &rhs) {
            return;
        }
        if is_invalid_syntax_argument(&rhs) {
            return;
        }

        let mut edits = vec![Edit::replace(dot, b" ".to_vec())];
        wrap_in_parentheses_if_chained(ctx, &call, &mut edits);
        if let Some(selector) = call.message_loc() {
            if insert_space_after(ctx, &call, &rhs, selector.span()) {
                edits.push(Edit::insert(selector.span().end, b" ".to_vec()));
            }
        }

        ctx.report_with_fix(
            &Self::META,
            dot,
            MSG,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// `foo.~@` and `foo.!@` call the method `~`/`!`, but are written with an
/// extra `@` the plain operator name does not have -- RuboCop's
/// `method_name.to_s != selector.source` check, read here as "the written
/// selector text doesn't match the resolved method name".
fn unary_method_no_operator(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    let Some(message_loc) = call.message_loc() else { return false };
    ctx.text(message_loc.span()) != call.name().as_slice()
}

/// The call's sole argument, folding a trailing `&block`/`&:sym` pass (held
/// in `block()` as a [`ruby_ast::BlockArgumentNode`] rather than
/// `arguments()`, see the module docs) back into the count.
fn single_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let regular = call.arguments().map(|a| a.arguments());
    let regular_count = regular.as_ref().map_or(0, |list| list.iter().count());
    let block_pass = call.block().filter(|b| b.as_block_argument_node().is_some());
    match (regular_count, block_pass) {
        (1, None) => regular.and_then(|list| list.iter().next()),
        (0, Some(pass)) => Some(pass),
        _ => None,
    }
}

/// `INVALID_SYNTAX_ARG_TYPES`: a splat, forwarded-args (`...`), or
/// block-pass argument -- none of these are valid as the RHS of a binary
/// operator expression, so the dot must stay. A keyword hash whose first
/// element is a double-splat (`**kw`, with or without a name) is the same
/// check applied to `argument.children.first&.type` for a `hash`-shaped
/// argument.
fn is_invalid_syntax_argument(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::SplatNode | NodeKind::ForwardingArgumentsNode | NodeKind::BlockArgumentNode => {
            true
        }
        NodeKind::KeywordHashNode => node
            .as_keyword_hash_node()
            .and_then(|h| h.elements().iter().next())
            .is_some_and(|first| first.kind() == NodeKind::AssocSplatNode),
        _ => false,
    }
}

/// Checks for an acceptable case of `foo.+(bar).baz`: the call is chained
/// into a further method call, and its sole argument is itself wrapped in
/// explicit parentheses and is not a bare (receiver-less) method call --
/// RuboCop's `argument.children.first && argument.parent.parenthesized?`,
/// where `argument.children.first` is falsy only for a `send` node with a
/// nil receiver.
fn method_call_with_parenthesized_arg(
    ctx: &Context<'_>,
    call: &CallNode<'_>,
    rhs: &Node<'_>,
) -> bool {
    if ctx.parent().is_none_or(|p| p.kind != NodeKind::CallNode) {
        return false;
    }
    let rhs_is_bare_call = rhs.as_call_node().is_some_and(|c| c.receiver().is_none());
    if rhs_is_bare_call {
        return false;
    }
    call.opening_loc().is_some()
}

/// `wrap_in_parentheses_if_chained`: when this call is itself used as the
/// *receiver* of a further method call (as opposed to being passed as one
/// of that call's arguments), wrap the whole call in parentheses so
/// replacing its dot with a space does not change what the chain applies
/// to. A call used as a receiver always starts at the same byte offset as
/// its parent (see the module docs); an unequal start means `call` was
/// passed as an argument instead, so no wrap is needed.
fn wrap_in_parentheses_if_chained(ctx: &Context<'_>, call: &CallNode<'_>, edits: &mut Vec<Edit>) {
    let Some(parent) = ctx.parent() else { return };
    if parent.kind != NodeKind::CallNode {
        return;
    }
    if parent.span.start != call.as_node().span().start {
        return;
    }
    let node_span = call.as_node().span();
    // `ParenthesesCorrector.correct`: strip the call's own existing
    // argument parentheses (and surrounding space) before re-wrapping the
    // whole call, else they would be left stranded inside the new parens.
    if let (Some(opening), Some(closing)) = (call.opening_loc(), call.closing_loc()) {
        edits.push(Edit::delete(ctx.with_surrounding_space(
            opening.span(),
            Side::Right,
            true,
            true,
        )));
        edits.push(Edit::delete(closing.span()));
    }
    if let Some(selector) = call.message_loc() {
        edits.push(Edit::insert(selector.span().end, b" ".to_vec()));
    }
    edits.push(Edit::insert(node_span.start, b"(".to_vec()));
    edits.push(Edit::insert(node_span.end, b")".to_vec()));
}

/// `insert_space_after?`.
fn insert_space_after(
    ctx: &Context<'_>,
    call: &CallNode<'_>,
    rhs: &Node<'_>,
    selector: Span,
) -> bool {
    if selector.end == rhs.span().start {
        return true;
    }
    if ctx.parent().is_some_and(|p| p.kind == NodeKind::CallNode) {
        // If chained, a space is already added by `wrap_in_parentheses_if_chained`.
        return false;
    }
    // For `/` operations, if the RHS starts with a `(` without space, add
    // one to avoid a syntax error.
    if call.name().as_slice() == b"/" && ctx.text(Span::new(selector.end, rhs.span().start)) == b"("
    {
        return true;
    }
    false
}
