//! `Style/ComparableClamp`, ported from RuboCop's
//! `lib/rubocop/cop/style/comparable_clamp.rb`, plus the `Alignment` mixin
//! it includes.
//!
//! # `if_elsif_else_condition?`
//!
//! Upstream's eight pattern alternatives all require the exact same shape
//! (`if COND1 BOUND1 (if COND2 BOUND2 (else X))`, i.e. a plain `if` whose
//! `elsif` is itself matched, terminated by a plain `else`), differing only
//! in which side of `<`/`>` each operand sits on and which bound (the
//! "low"/`min` one or the "high"/`max` one) the `if` branch checks first.
//! Several placeholders are *repeated* within one alternative (`_min`/`_max`
//! appearing once in a condition and once as a branch body; `_x` appearing
//! in both conditions and the final `else`), which in RuboCop's
//! node-pattern DSL requires the repeated occurrences to be the exact same
//! node -- i.e. the condition's non-`x` operand must literally be the
//! branch body, and the shared variable in both conditions must literally
//! be the `else` body. [`condition_matches`] enforces both constraints
//! directly (by source-text equality) instead of via sixteen separate
//! pattern literals, and returns which of the branch's own two bound
//! readings it is (`min`, true, vs. `max`, false).
//!
//! # Autocorrection
//!
//! `node.elsif?` (an `if` reached as the chain's second branch, not its
//! top) inserts a new `else\n` immediately before `node` and replaces
//! `node`'s own span (which runs through the original chain's `end`,
//! swallowing it) with `#{indentation}#{prefer}`; since both edits touch
//! the same boundary, this port folds them into one replacement of
//! `node`'s span with `else\n#{indentation}#{prefer}` instead of two
//! separate (and here, overlapping) edits. [`indentation`] mirrors the
//! `Alignment` mixin's `indent`-style column (`node`'s own column plus one
//! configured `Layout/IndentationWidth::Width`, falling back to `2`) -- the
//! only existing fixture for the `elsif` path needs the inserted statement
//! indented one level past the chain's own column, not aligned with it. A
//! non-`elsif` top-level `if` is replaced outright.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ArrayNode, CallNode, ElseNode, IfNode};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG_TEMPLATE: &str = "Use `%<prefer>s` instead of `if/elsif/else`.";
/// RuboCop's `MSG_MIN_MAX`.
const MSG_MIN_MAX: &str = "Use `Comparable#clamp` instead.";

/// Enforces the use of `Comparable#clamp` instead of comparison by minimum and maximum.
#[derive(Debug, Clone)]
pub struct ComparableClamp {
    /// `Layout/IndentationWidth`'s configured `Width`, falling back to `2`.
    indentation_width: u32,
}

impl Rule for ComparableClamp {
    const META: RuleMeta = RuleMeta {
        name: "Style/ComparableClamp",
        department: Department::Style,
        summary:
            "Enforces the use of `Comparable#clamp` instead of comparison by minimum and maximum.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .and_then(|w| u32::try_from(w).ok())
            .unwrap_or(2);
        Ok(Self { indentation_width: width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::IfNode { .. } => {
                let Some(if_node) = node.as_if_node() else { return };
                self.check_if(&if_node, node, ctx);
            }
            Node::CallNode { .. } => {
                let Some(call) = node.as_call_node() else { return };
                if is_array_min_max(&call) {
                    ctx.report(&Self::META, node.span(), MSG_MIN_MAX);
                }
            }
            _ => {}
        }
    }
}

impl ComparableClamp {
    fn check_if(&self, if_node: &IfNode<'_>, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(shape) = if_elsif_else_shape(if_node, ctx) else { return };
        let (min_src, max_src) = if shape.if_is_min {
            (ctx.text(shape.if_body.span()), ctx.text(shape.elsif_body.span()))
        } else {
            (ctx.text(shape.elsif_body.span()), ctx.text(shape.if_body.span()))
        };
        let prefer = format!(
            "{}.clamp({}, {})",
            parenthesize_if_needed(&shape.else_body, ctx),
            String::from_utf8_lossy(min_src),
            String::from_utf8_lossy(max_src),
        );
        let message = MSG_TEMPLATE.replace("%<prefer>s", &prefer);

        let is_elsif = ctx.parent().is_some_and(|p| p.kind == NodeKind::IfNode);
        // An `elsif`-chain `IfNode`'s own span runs through the shared
        // `end` keyword (there is no separate `end` for a nested elsif), so
        // the replacement must stop at the `else` branch's own last
        // statement instead of consuming it.
        let edit_span = if is_elsif {
            ruby_source::Span::new(node.span().start, shape.else_body.span().end)
        } else {
            node.span()
        };
        let replacement = if is_elsif {
            let column = ctx.line_col(node.span().start).column;
            let indent = " ".repeat((column + self.indentation_width) as usize);
            format!("else\n{indent}{prefer}")
        } else {
            prefer
        };
        let edits = vec![Edit::replace(edit_span, replacement.into_bytes())];

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// The matched `if`/`elsif`/`else` shape: the `if` and `elsif` branch
/// bodies (whichever reads as `min`/`max`, tracked by `if_is_min`) and the
/// `else` branch body (the clamped variable itself).
struct Shape<'pr> {
    if_body: Node<'pr>,
    elsif_body: Node<'pr>,
    else_body: Node<'pr>,
    if_is_min: bool,
}

/// RuboCop's `if_elsif_else_condition?` plus the role assignment
/// `min_condition?` otherwise provides separately.
fn if_elsif_else_shape<'pr>(if_node: &IfNode<'pr>, ctx: &Context<'_>) -> Option<Shape<'pr>> {
    let if_body = sole_statement(if_node.statements())?;
    let elsif = if_node.subsequent()?.as_if_node()?;
    let elsif_body = sole_statement(elsif.statements())?;
    let else_node: ElseNode<'pr> = elsif.subsequent()?.as_else_node()?;
    let else_body = sole_statement(else_node.statements())?;

    let x_src = ctx.text(else_body.span());
    let if_is_min = condition_matches(if_node.predicate(), ctx.text(if_body.span()), x_src, ctx)?;
    let elsif_is_min =
        condition_matches(elsif.predicate(), ctx.text(elsif_body.span()), x_src, ctx)?;
    if if_is_min == elsif_is_min {
        return None;
    }
    Some(Shape { if_body, elsif_body, else_body, if_is_min })
}

/// The lone statement of `statements`, unwrapping Prism's always-present
/// `StatementsNode`.
fn sole_statement(statements: Option<ruby_ast::node::StatementsNode<'_>>) -> Option<Node<'_>> {
    let body = statements?.body();
    if body.len() != 1 {
        return None;
    }
    body.iter().next()
}

/// Whether `predicate` asserts `x < body` (returns `Some(true)`, `body` is
/// `min`) or `body < x` (returns `Some(false)`, `body` is `max`), written as
/// either `<` or `>` with either operand order.
fn condition_matches(
    predicate: Node<'_>,
    body: &[u8],
    x: &[u8],
    ctx: &Context<'_>,
) -> Option<bool> {
    let call = predicate.as_call_node()?;
    let name = call.name().as_slice();
    if name != b"<" && name != b">" {
        return None;
    }
    let lhs = ctx.text(call.receiver()?.span());
    let args = call.arguments()?.arguments();
    if args.len() != 1 {
        return None;
    }
    let rhs = ctx.text(args.iter().next()?.span());

    match name {
        b"<" if lhs == x && rhs == body => Some(true),
        b"<" if lhs == body && rhs == x => Some(false),
        b">" if lhs == body && rhs == x => Some(true),
        b">" if lhs == x && rhs == body => Some(false),
        _ => None,
    }
}

/// `array_min_max?`: `[A, B].min` where `A` or `B` is itself `[_, _].max`
/// (or the `max`/`min` mirror), each inner array having exactly 2 elements.
fn is_array_min_max(call: &CallNode<'_>) -> bool {
    let outer_name = call.name().as_slice();
    let Some(inner_name): Option<&[u8]> = (match outer_name {
        b"min" => Some(b"max"),
        b"max" => Some(b"min"),
        _ => None,
    }) else {
        return false;
    };
    if call.arguments().is_some_and(|a| !a.arguments().is_empty()) {
        return false;
    }
    let Some(array) = call.receiver().and_then(|r| r.as_array_node()) else { return false };
    let elements = array.elements();
    if elements.len() != 2 {
        return false;
    }
    elements.iter().any(|el| is_two_element_array_call(&el, inner_name))
}

/// `el` is a call named `name` (no args) on a 2-element array literal.
fn is_two_element_array_call(el: &Node<'_>, name: &[u8]) -> bool {
    let Some(call) = el.as_call_node() else { return false };
    if call.name().as_slice() != name || call.arguments().is_some_and(|a| !a.arguments().is_empty())
    {
        return false;
    }
    call.receiver()
        .and_then(|r| r.as_array_node())
        .is_some_and(|inner: ArrayNode<'_>| inner.elements().len() == 2)
}

/// `parenthesize_if_needed`.
fn parenthesize_if_needed(node: &Node<'_>, ctx: &Context<'_>) -> String {
    let needs_parens = matches!(
        node.kind(),
        NodeKind::AndNode
            | NodeKind::OrNode
            | NodeKind::IfNode
            | NodeKind::UnlessNode
            | NodeKind::RangeNode
    ) || is_assignment_kind(node.kind())
        || node.as_call_node().is_some_and(|c| is_operator_or_unary(&c));
    let source = ctx.text(node.span());
    if needs_parens {
        format!("({})", String::from_utf8_lossy(source))
    } else {
        String::from_utf8_lossy(source).into_owned()
    }
}

/// `node.operator_method? || node.unary_operation?`: a `send`/`csend` using
/// a symbolic binary operator (receiver plus one argument) or a prefix
/// unary operator (receiver, no arguments).
fn is_operator_or_unary(call: &CallNode<'_>) -> bool {
    if call.receiver().is_none() {
        return false;
    }
    let name = call.name().as_slice();
    let arg_count = call.arguments().map_or(0, |a| a.arguments().len());
    match arg_count {
        0 => matches!(name, b"-@" | b"+@" | b"!" | b"~"),
        1 => matches!(
            name,
            b"+" | b"-"
                | b"*"
                | b"/"
                | b"%"
                | b"**"
                | b"=="
                | b"==="
                | b"!="
                | b"<"
                | b">"
                | b"<="
                | b">="
                | b"<=>"
                | b"&"
                | b"|"
                | b"^"
                | b"<<"
                | b">>"
                | b"=~"
                | b"!~"
                | b"[]"
        ),
        _ => false,
    }
}

/// RuboCop-AST's `Node::ASSIGNMENTS`, mapped onto Prism's per-target-kind
/// `*WriteNode` family (copied privately from `style/and_or.rs`).
fn is_assignment_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::MultiWriteNode
            | NodeKind::CallOperatorWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::CallOrWriteNode
            | NodeKind::IndexOperatorWriteNode
            | NodeKind::IndexAndWriteNode
            | NodeKind::IndexOrWriteNode
    )
}
