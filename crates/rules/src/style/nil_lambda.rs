//! `Style/NilLambda`, ported from RuboCop's
//! `lib/rubocop/cop/style/nil_lambda.rb`.
//!
//! Handles both of Prism's two shapes for "a lambda or proc": a stabby
//! `-> { ... }` is its own `LambdaNode` (with `body` directly, no owning
//! call); `lambda { ... }`, `proc { ... }`, and `Proc.new { ... }` are all a
//! `CallNode` whose attached block (`CallNode#block`, downcast to a real
//! `BlockNode`, never a `&blk` `BlockArgumentNode`) carries the body.
//! `nil_return?`'s whitequark pattern `{ ({return next break} nil) (nil) }`
//! becomes [`nil_return_kind`]: the body's sole statement is either a bare
//! `nil` literal, or a `return`/`next`/`break` whose single argument is
//! `nil`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::ArgumentsNode;
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// RuboCop's `MSG`.
const MSG: &str = "Use an empty %<type>s instead of always returning nil.";

/// Prefer `-> {}` to `-> { nil }`.
#[derive(Debug, Clone)]
pub struct NilLambda;

impl Rule for NilLambda {
    const META: RuleMeta = RuleMeta {
        name: "Style/NilLambda",
        department: Department::Style,
        summary: "Prefer `-> {}` to `-> { nil }`.",
        explanation: "Checks for lambdas and procs that always return nil, which can be \
            replaced with an empty lambda or proc instead.\n\nNOTE: A `proc` that returns nil \
            via an explicit `return` is allowed, because in a `proc` `return` exits the \
            enclosing method, so removing it would change behavior. A lambda is still reported, \
            since there `return` only exits the lambda itself.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::LambdaNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(lambda) = node.as_lambda_node() {
            check(ctx, node.span(), lambda.body(), true);
            return;
        }
        let call = node.as_call_node().expect("kind matched");
        if !ext::is_lambda_or_proc(&call) {
            return;
        }
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        let is_lambda = call.name().as_slice() == b"lambda";
        check(ctx, node.span(), block.body(), is_lambda);
    }
}

/// `on_block`'s body: `nil_return?` plus the `return`-in-a-proc exception,
/// then the report and its autocorrect.
fn check(ctx: &mut Context<'_>, whole: Span, body: Option<Node<'_>>, is_lambda: bool) {
    let Some(kind) = nil_return_kind(body) else { return };
    if kind == ReturnKind::Return && !is_lambda {
        return;
    }

    let message = MSG.replace("%<type>s", if is_lambda { "lambda" } else { "proc" });
    let body_span = body.expect("nil_return_kind matched").span();
    let remove = if ctx.is_single_line(whole) {
        ctx.with_surrounding_space(body_span, Side::Both, true, false)
    } else {
        ctx.whole_lines(body_span)
    };
    ctx.report_with_fix(
        &NilLambda::META,
        whole,
        message,
        Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(remove)] },
    );
}

/// Which "nil return" shape a body's single statement is, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReturnKind {
    /// A bare `nil` literal.
    Bare,
    /// `return nil`.
    Return,
    /// `next nil`.
    Next,
    /// `break nil`.
    Break,
}

/// `nil_return?`: `body` is present, is a single statement, and that
/// statement is a bare `nil` literal or a `return`/`next`/`break` whose sole
/// argument is `nil`.
fn nil_return_kind(body: Option<Node<'_>>) -> Option<ReturnKind> {
    let stmt = single_statement(&body?)?;
    if stmt.as_nil_node().is_some() {
        return Some(ReturnKind::Bare);
    }
    if let Some(r) = stmt.as_return_node() {
        return is_nil_arg(r.arguments()).then_some(ReturnKind::Return);
    }
    if let Some(n) = stmt.as_next_node() {
        return is_nil_arg(n.arguments()).then_some(ReturnKind::Next);
    }
    if let Some(b) = stmt.as_break_node() {
        return is_nil_arg(b.arguments()).then_some(ReturnKind::Break);
    }
    None
}

/// Does `args` hold exactly one argument, and is it a `nil` literal?
fn is_nil_arg(args: Option<ArgumentsNode<'_>>) -> bool {
    let Some(args) = args else { return false };
    let list = args.arguments();
    list.len() == 1 && list.first().is_some_and(|n| n.as_nil_node().is_some())
}

/// A body's sole statement, if it has exactly one.
fn single_statement<'pr>(body: &Node<'pr>) -> Option<Node<'pr>> {
    let statements = body.as_statements_node()?;
    let list = statements.body();
    (list.len() == 1).then(|| list.first()).flatten()
}
