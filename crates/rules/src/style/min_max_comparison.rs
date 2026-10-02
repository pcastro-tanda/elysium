//! `Style/MinMaxComparison`, ported from RuboCop's
//! `lib/rubocop/cop/style/min_max_comparison.rb`.
//!
//! whitequark's single `:if` node (covering ternary, modifier `if`, plain
//! `if`, and -- recursively through its own `else_branch` -- an `elsif`
//! link) is Prism's `IfNode`/`UnlessNode` pair: a ternary has no
//! `if_keyword_loc`, a modifier form has no `end_keyword_loc`, an `elsif`
//! link is reached through `IfNode::subsequent` (`Some(Node::IfNode)`, whose
//! own `if_keyword_loc` reads `"elsif"`), and `unless` is its own node kind
//! rather than an `if` with swapped branches -- the branch swap upstream
//! does by hand for `node.unless?` is done here by reading `UnlessNode`'s
//! `statements` (the false branch) and `else_clause` (the true branch) in
//! that order instead. Each `elsif` link is reached as an ordinary
//! descendant of the top `if` by the generic tree walk, so it is inspected
//! on its own, exactly as upstream's `on_if` fires once per nested `:if`
//! node.
//!
//! `node.loc.else` for an `elsif` link (used by upstream's autocorrection)
//! is the same byte position as that link's own `if_keyword_loc` (whitequark
//! attaches the "elsif" token's location to the inner node as its *opening*
//! keyword and to the outer node as its *else* marker); `node.parent.loc.else
//! .begin_pos` is therefore just `node.if_keyword_loc().span().start` here,
//! sidestepping the parent lookup entirely.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Comparison operators this cop reacts to, split by preference direction.
const GREATER_OPERATORS: [&[u8]; 2] = [b">", b">="];
const LESS_OPERATORS: [&[u8]; 2] = [b"<", b"<="];

/// Enforces the use of `max` or `min` instead of comparison for greater or less.
#[derive(Debug, Clone)]
pub struct MinMaxComparison;

impl Rule for MinMaxComparison {
    const META: RuleMeta = RuleMeta {
        name: "Style/MinMaxComparison",
        department: Department::Style,
        summary: "Enforces the use of `max` or `min` instead of comparison for greater or less.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(shape) = Shape::of(node) else { return };

        let predicate = unwrap_parens(shape.predicate);
        let Some(call) = predicate.as_call_node() else { return };
        let operator = call.name();
        let operator = operator.as_slice();
        if !GREATER_OPERATORS.contains(&operator) && !LESS_OPERATORS.contains(&operator) {
            return;
        }
        let Some(lhs) = call.receiver() else { return };
        let Some(rhs) = call.arguments().and_then(|a| a.arguments().first()) else { return };
        if call.arguments().is_some_and(|a| a.arguments().len() != 1) {
            return;
        }

        let (if_branch, else_branch) =
            if shape.is_unless { (shape.second, shape.first) } else { (shape.first, shape.second) };
        let Some(if_branch) = if_branch else { return };
        let Some(else_branch) = else_branch else { return };

        let lhs_text = ctx.text(lhs.span());
        let rhs_text = ctx.text(rhs.span());
        let if_text = ctx.text(if_branch);
        let else_text = ctx.text(else_branch);

        let preferred = if lhs_text == if_text && rhs_text == else_text {
            if GREATER_OPERATORS.contains(&operator) {
                "max"
            } else {
                "min"
            }
        } else if lhs_text == else_text && rhs_text == if_text {
            if LESS_OPERATORS.contains(&operator) {
                "max"
            } else {
                "min"
            }
        } else {
            return;
        };

        let lhs_src = String::from_utf8_lossy(lhs_text);
        let rhs_src = String::from_utf8_lossy(rhs_text);
        let replacement = format!("[{lhs_src}, {rhs_src}].{preferred}");
        let message = format!("Use `{replacement}` instead.");

        let edits = if shape.elsif {
            let Some(else_keyword_start) = shape.else_keyword_start else { return };
            let keyword_start = shape.keyword_loc.map_or(node.span().start, |loc| loc.span().start);
            vec![
                Edit::delete(Span::new(keyword_start, else_keyword_start)),
                Edit::replace(else_branch, replacement.clone().into_bytes()),
            ]
        } else {
            vec![Edit::replace(node.span(), replacement.clone().into_bytes())]
        };

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// Unified view over `IfNode`/`UnlessNode`, normalising `unless`'s reversed
/// branch order away: `first`/`second` are always the structurally first-
/// and second-written bodies (before upstream's `if node.unless?` swap).
struct Shape<'pr> {
    predicate: Node<'pr>,
    first: Option<Span>,
    second: Option<Span>,
    is_unless: bool,
    elsif: bool,
    keyword_loc: Option<ruby_ast::Location<'pr>>,
    else_keyword_start: Option<u32>,
}

impl<'pr> Shape<'pr> {
    fn of(node: &Node<'pr>) -> Option<Self> {
        if let Some(if_node) = node.as_if_node() {
            let elsif = if_node.if_keyword_loc().is_some_and(|loc| loc.as_slice() == b"elsif");
            let (second, else_keyword_start) = match if_node.subsequent() {
                Some(Node::ElseNode { .. }) => {
                    let else_node = if_node.subsequent().and_then(|n| n.as_else_node())?;
                    (
                        else_node.statements().map(|s| s.as_node().span()),
                        Some(else_node.else_keyword_loc().span().start),
                    )
                }
                _ => (None, None),
            };
            Some(Self {
                predicate: if_node.predicate(),
                first: if_node.statements().map(|s| s.as_node().span()),
                second,
                is_unless: false,
                elsif,
                keyword_loc: if_node.if_keyword_loc(),
                else_keyword_start,
            })
        } else if let Some(unless_node) = node.as_unless_node() {
            let (second, else_keyword_start) = match unless_node.else_clause() {
                Some(else_node) => (
                    else_node.statements().map(|s| s.as_node().span()),
                    Some(else_node.else_keyword_loc().span().start),
                ),
                None => (None, None),
            };
            Some(Self {
                predicate: unless_node.predicate(),
                first: unless_node.statements().map(|s| s.as_node().span()),
                second,
                is_unless: true,
                elsif: false,
                keyword_loc: None,
                else_keyword_start,
            })
        } else {
            None
        }
    }
}

/// RuboCop's `(begin (send ...))` alternative: an explicitly parenthesized
/// condition. Prism wraps a parenthesized expression's body in a
/// `StatementsNode` even for a single statement (whitequark elides it), so
/// the last statement is taken, mirroring `deparenthesize`'s
/// `node.children.last` loop elsewhere in this crate.
fn unwrap_parens(node: Node<'_>) -> Node<'_> {
    if let Some(paren) = node.as_parentheses_node() {
        if let Some(body) = paren.body() {
            if let Some(statements) = body.as_statements_node() {
                if let Some(last) = statements.body().last() {
                    return last;
                }
            }
            return body;
        }
    }
    node
}
