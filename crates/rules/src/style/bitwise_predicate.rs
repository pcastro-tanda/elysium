//! `Style/BitwisePredicate`, ported from RuboCop's
//! `lib/rubocop/cop/style/bitwise_predicate.rb`.
//!
//! Upstream's four node-matchers all require the call's receiver to be a
//! parenthesized `_ & _` (`bit_operation?`, matched against `(begin (send _
//! :& _))`); Prism's [`ruby_ast::node::ParenthesesNode`] is the direct
//! counterpart of whitequark's `begin` wrapping one expression (unlike
//! `BeginNode`, which only ever appears for a `kwbegin`/rescue body in this
//! codebase's Prism mapping -- see `redundant_begin.rs`). [`bit_operation`]
//! therefore unwraps a `ParenthesesNode` whose sole body is a `&` call.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG_TEMPLATE: &str = "Replace with `%<preferred>s` for comparison with bit flags.";

/// Prefer bitwise predicate methods over direct comparison operations.
#[derive(Debug, Clone)]
pub struct BitwisePredicate;

impl Rule for BitwisePredicate {
    const META: RuleMeta = RuleMeta {
        name: "Style/BitwisePredicate",
        department: Department::Style,
        summary: "Prefer bitwise predicate methods over direct comparison operations.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
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
        if call.is_safe_navigation() {
            return;
        }
        let method = call.name().as_slice();
        if !matches!(method, b"!=" | b"==" | b">" | b">=" | b"positive?" | b"zero?") {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(bit_op) = bit_operation(&receiver) else { return };

        let Some(preferred) = preferred_method(&call, method, &bit_op, ctx) else { return };

        let message = MSG_TEMPLATE.replace("%<preferred>s", &preferred);
        let edits = vec![Edit::replace(node.span(), preferred.into_bytes())];
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// `bit_operation?`: `receiver` is a parenthesized `_ & _` call -- RuboCop's
/// `(begin (send _ :& _))`, unwrapped to the inner `&` call.
fn bit_operation<'pr>(receiver: &Node<'pr>) -> Option<CallNode<'pr>> {
    let parens = receiver.as_parentheses_node()?;
    let inner = parens.body()?;
    let stmts = inner.as_statements_node()?;
    let body = stmts.body();
    if body.len() != 1 {
        return None;
    }
    let call = body.iter().next()?.as_call_node()?;
    if call.is_safe_navigation() || call.name().as_slice() != b"&" {
        return None;
    }
    Some(call)
}

/// Dispatches to the one preferred-method check the outer method name
/// permits, mirroring `preferred_method`'s `if/elsif` chain (`anybits?` /
/// `allbits?` / `nobits?`, checked in that order).
fn preferred_method(
    outer: &CallNode<'_>,
    method: &[u8],
    bit_op: &CallNode<'_>,
    ctx: &Context<'_>,
) -> Option<String> {
    let lhs = bit_op.receiver()?;
    let rhs = bit_op.arguments()?.arguments().iter().next()?;
    let first_arg = outer.arguments().and_then(|a| a.arguments().iter().next());

    if is_anybits(method, outer, first_arg.as_ref()) {
        let lhs_src = ctx.text(lhs.span());
        let rhs_src = ctx.text(rhs.span());
        return Some(format!(
            "{}.anybits?({})",
            String::from_utf8_lossy(lhs_src),
            String::from_utf8_lossy(rhs_src)
        ));
    }
    if is_allbits(method, outer, &lhs, &rhs, ctx) {
        let lhs_src = ctx.text(lhs.span());
        let rhs_src = ctx.text(rhs.span());
        let first_arg_src = first_arg.as_ref().map(|a| ctx.text(a.span()));
        return Some(if first_arg_src == Some(lhs_src) {
            format!(
                "{}.allbits?({})",
                String::from_utf8_lossy(rhs_src),
                String::from_utf8_lossy(lhs_src)
            )
        } else {
            format!(
                "{}.allbits?({})",
                String::from_utf8_lossy(lhs_src),
                String::from_utf8_lossy(rhs_src)
            )
        });
    }
    if is_nobits(method, outer, first_arg.as_ref()) {
        let lhs_src = ctx.text(lhs.span());
        let rhs_src = ctx.text(rhs.span());
        return Some(format!(
            "{}.nobits?({})",
            String::from_utf8_lossy(lhs_src),
            String::from_utf8_lossy(rhs_src)
        ));
    }
    None
}

/// `anybits?`: `#positive?`, `> 0`, `>= 1`, or `!= 0`.
fn is_anybits(method: &[u8], outer: &CallNode<'_>, first_arg: Option<&Node<'_>>) -> bool {
    match method {
        b"positive?" => outer.arguments().is_none_or(|a| a.arguments().is_empty()),
        b">" | b"!=" => is_int_literal(first_arg, 0),
        b">=" => is_int_literal(first_arg, 1),
        _ => false,
    }
}

/// `nobits?`: `#zero?` or `== 0`.
fn is_nobits(method: &[u8], outer: &CallNode<'_>, first_arg: Option<&Node<'_>>) -> bool {
    match method {
        b"zero?" => outer.arguments().is_none_or(|a| a.arguments().is_empty()),
        b"==" => is_int_literal(first_arg, 0),
        _ => false,
    }
}

/// `allbits?`: `== flags`, where `flags` is literally the same source as
/// one side of the `&` (either side, per the two `allbits?` pattern
/// alternatives).
fn is_allbits(
    method: &[u8],
    outer: &CallNode<'_>,
    lhs: &Node<'_>,
    rhs: &Node<'_>,
    ctx: &Context<'_>,
) -> bool {
    if method != b"==" {
        return false;
    }
    let Some(args) = outer.arguments() else { return false };
    let args = args.arguments();
    if args.len() != 1 {
        return false;
    }
    let arg_src = ctx.text(args.iter().next().expect("len == 1").span());
    arg_src == ctx.text(lhs.span()) || arg_src == ctx.text(rhs.span())
}

/// `node` is a literal integer equal to `value`.
fn is_int_literal(node: Option<&Node<'_>>, value: i32) -> bool {
    node.and_then(Node::as_integer_node)
        .and_then(|i| i.value().try_into().ok())
        .is_some_and(|v: i32| v == value)
}
