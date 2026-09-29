//! `Style/NegatedWhile`, ported from RuboCop's
//! `lib/rubocop/cop/style/negated_while.rb`, which mixes in the shared
//! `NegativeConditional` module (`lib/rubocop/cop/mixin/negative_conditional.rb`).
//!
//! Whitequark's `begin` node (any parenthesized/grouped condition) is
//! Prism's `ParenthesesNode` wrapping a `StatementsNode`; `condition =
//! condition.children.last while condition.begin_type?` becomes a loop that
//! unwraps nested `ParenthesesNode`s down to their last statement, and an
//! empty `()` (`ParenthesesNode` with no `body`) reproduces
//! `empty_condition?`'s `(begin)` match by returning `None` on the way in.
//! `!x`/`not x` both parse as a `CallNode` named `!` with `x` as its
//! `receiver`; `single_negative?`'s exclusion of `(send _ :!)` receivers
//! rules out the doubly-negated `!!x` case.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Favor until over while for negative conditions.
#[derive(Debug, Clone)]
pub struct NegatedWhile;

impl Rule for NegatedWhile {
    const META: RuleMeta = RuleMeta {
        name: "Style/NegatedWhile",
        department: Department::Style,
        summary: "Checks for uses of while with a negated condition.",
        explanation: "Checks for uses of `while` with a negated condition.\n\n```ruby\n# bad\nwhile !foo\n  bar\nend\n\n# good\nuntil foo\n  bar\nend\n\n# bad\nbar until !foo\n\n# good\nbar while foo\nbar while !foo && baz\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::WhileNode, NodeKind::UntilNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::WhileNode => {
                let w = node.as_while_node().expect("kind matched");
                // `begin ... end while cond` is whitequark's `while_post`, a
                // distinct node type upstream's `on_while` never subscribes
                // to; Prism instead reuses `WhileNode` with this flag set.
                if w.is_begin_modifier() {
                    return;
                }
                check_negative_conditional(
                    ctx,
                    node.span(),
                    w.keyword_loc().span(),
                    w.predicate(),
                    "while",
                    "until",
                );
            }
            NodeKind::UntilNode => {
                let u = node.as_until_node().expect("kind matched");
                // See the `WhileNode` arm: `begin ... end until cond` is
                // whitequark's `until_post`, never reaching `on_until`.
                if u.is_begin_modifier() {
                    return;
                }
                check_negative_conditional(
                    ctx,
                    node.span(),
                    u.keyword_loc().span(),
                    u.predicate(),
                    "until",
                    "while",
                );
            }
            _ => {}
        }
    }
}

/// RuboCop's `NegativeConditional#check_negative_conditional`, specialized
/// to `while`/`until` (the `if_type? && node.else?` guard in the shared
/// module never applies to a loop, so it is omitted).
fn check_negative_conditional(
    ctx: &mut Context<'_>,
    node_span: Span,
    keyword_span: Span,
    predicate: Node<'_>,
    current_keyword: &str,
    inverse_keyword: &str,
) {
    let Some(condition) = unwrap_negated_condition(predicate) else { return };
    let Some(call) = single_negative(&condition) else { return };
    let message =
        format!("Favor `{inverse_keyword}` over `{current_keyword}` for negative conditions.");
    // RuboCop's `ConditionCorrector.correct_negative_condition`: replace the
    // loop keyword with its inverse, and the negated condition with its
    // receiver's own source (`condition.children.first.source`).
    let receiver = call.receiver().expect("unary `!`/`not` always has a receiver");
    let receiver_text = ctx.text(receiver.span()).to_vec();
    ctx.report_with_fix(
        &NegatedWhile::META,
        node_span,
        message,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![
                Edit::replace(keyword_span, inverse_keyword.as_bytes().to_vec()),
                Edit::replace(condition.span(), receiver_text),
            ],
        },
    );
}

/// RuboCop's `condition = condition.children.last while condition.begin_type?`,
/// preceded by `return if empty_condition?(condition)`: an empty `()` at any
/// unwrapping step (a `ParenthesesNode` with no `body`) is `(begin)`'s empty
/// match, so it short-circuits to `None` here exactly as the early return
/// upstream does.
fn unwrap_negated_condition(mut node: Node<'_>) -> Option<Node<'_>> {
    while let Some(paren) = node.as_parentheses_node() {
        let body = paren.body()?;
        let stmts = body.as_statements_node()?;
        node = stmts.body().last()?;
    }
    Some(node)
}

/// RuboCop's `single_negative?`: `(send !(send _ :!) :!)` -- a `!`/`not`
/// call whose receiver is not itself a `!`/`not` call (which would make the
/// condition doubly negated, e.g. `!!foo`).
fn single_negative<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"!" {
        return None;
    }
    if call
        .receiver()
        .is_some_and(|r| r.as_call_node().is_some_and(|c| c.name().as_slice() == b"!"))
    {
        return None;
    }
    Some(call)
}
