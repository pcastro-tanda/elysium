//! `Style/NestedModifier`, ported from RuboCop's
//! `lib/rubocop/cop/style/nested_modifier.rb`.
//!
//! Whitequark's parser gives `if`, `unless`, `while`, and `until` all a
//! `basic_conditional?` predicate covering `:if`/`:while`/`:until` (`unless`
//! shares `:if`'s node type); Prism instead gives each of the four its own
//! kind. [`modifier_shape`] normalizes all four into one [`Modifier`], mirroring
//! rubocop-ast's `IfNode#keyword`/`#modifier_form?`/`#condition` (plus the
//! `WhileNode`/`UntilNode` equivalents).
//!
//! The upstream cop is written from the *inner* modifier's perspective
//! (`on_if`/`on_while`/`on_until` fire for every conditional node, and
//! `check(node)` looks at `node.parent`). Prism's visitor only hands rules
//! byte spans for ancestors (no full parent node), so this port instead
//! walks from the *outer* modifier: for each modifier-form node `X`, if `X`'s
//! single wrapped statement is itself a modifier-form conditional, that's
//! the inner/outer pair upstream would have found when visiting the inner
//! node directly. `ignore_node`'s dedup (skip an already-reported node's
//! descendants) becomes span containment, exactly as in
//! `style/multiline_if_modifier.rs`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, StatementsNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Avoid using nested modifiers.";

/// RuboCop-ast's `OPERATOR_METHODS` (`MethodIdentifierPredicates`).
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

/// RuboCop-ast's `Node::COMPARISON_OPERATORS`.
const COMPARISON_OPERATORS: &[&[u8]] = &[b"==", b"===", b"!=", b"<=", b">=", b">", b"<"];

/// A modifier-form `if`/`unless`/`while`/`until` node, normalized across all
/// four Prism kinds (rubocop-ast's `IfNode#keyword`/`#condition`/
/// `#modifier_form?`, plus the `WhileNode`/`UntilNode` equivalents).
struct Modifier<'pr> {
    /// RuboCop-ast's `#keyword` (`"if"`, `"unless"`, `"while"`, `"until"`).
    keyword: &'static str,
    keyword_span: Span,
    condition: Node<'pr>,
    /// The single statement executed under this modifier.
    body: Node<'pr>,
    /// RuboCop's `node.if_type?`: whitequark unifies `if` and `unless` under
    /// one `:if` node type, distinguishing `while`/`until` (their own
    /// types). True for [`NodeKind::IfNode`]/[`NodeKind::UnlessNode`].
    is_if_family: bool,
}

/// The lone statement of a `StatementsNode`, or `None` for zero or several
/// -- matching rubocop-ast's `node_parts`, which only omits the synthetic
/// `begin` wrapper (and so only unifies with a single statement).
fn single_statement(stmts: Option<StatementsNode<'_>>) -> Option<Node<'_>> {
    let stmts = stmts?;
    let mut items = stmts.body().iter();
    let first = items.next()?;
    if items.next().is_some() {
        return None;
    }
    Some(first)
}

/// Builds a [`Modifier`] for `node`, or `None` when it is not modifier-form
/// (RuboCop's `node.basic_conditional? && node.modifier_form?`): a real
/// `if`/`unless`/`while`/`until` keyword -- excluding `elsif` and ternaries,
/// which never carry an `if` keyword of their own, and excluding the
/// `begin ... end while`/`until` post-condition loop shape -- with no
/// closing `end`.
fn modifier_shape<'pr>(node: &Node<'pr>) -> Option<Modifier<'pr>> {
    match node {
        Node::IfNode { .. } => {
            let n = node.as_if_node().expect("kind matched");
            let kw_loc = n.if_keyword_loc()?;
            if kw_loc.as_slice() == b"elsif" || n.end_keyword_loc().is_some() {
                return None;
            }
            let body = single_statement(n.statements())?;
            Some(Modifier {
                keyword: "if",
                keyword_span: kw_loc.span(),
                condition: n.predicate(),
                body,
                is_if_family: true,
            })
        }
        Node::UnlessNode { .. } => {
            let n = node.as_unless_node().expect("kind matched");
            if n.end_keyword_loc().is_some() {
                return None;
            }
            let body = single_statement(n.statements())?;
            Some(Modifier {
                keyword: "unless",
                keyword_span: n.keyword_loc().span(),
                condition: n.predicate(),
                body,
                is_if_family: true,
            })
        }
        Node::WhileNode { .. } => {
            let n = node.as_while_node().expect("kind matched");
            if n.is_begin_modifier() || n.closing_loc().is_some() {
                return None;
            }
            let body = single_statement(n.statements())?;
            Some(Modifier {
                keyword: "while",
                keyword_span: n.keyword_loc().span(),
                condition: n.predicate(),
                body,
                is_if_family: false,
            })
        }
        Node::UntilNode { .. } => {
            let n = node.as_until_node().expect("kind matched");
            if n.is_begin_modifier() || n.closing_loc().is_some() {
                return None;
            }
            let body = single_statement(n.statements())?;
            Some(Modifier {
                keyword: "until",
                keyword_span: n.keyword_loc().span(),
                condition: n.predicate(),
                body,
                is_if_family: false,
            })
        }
        _ => None,
    }
}

/// RuboCop-ast's `SendNode#send_type?`, i.e. a plain (non-`&.`) method call.
fn as_plain_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    node.as_call_node().filter(|c| !c.is_safe_navigation())
}

fn is_operator_method(name: &[u8]) -> bool {
    OPERATOR_METHODS.contains(&name)
}

fn is_comparison_operator(name: &[u8]) -> bool {
    COMPARISON_OPERATORS.contains(&name)
}

/// RuboCop's `requires_parens?`.
fn requires_parens(condition: &Node<'_>, negated: bool) -> bool {
    let is_and_or = matches!(condition, Node::AndNode { .. } | Node::OrNode { .. });
    let is_or = matches!(condition, Node::OrNode { .. });
    let is_comparison =
        condition.as_call_node().is_some_and(|c| is_comparison_operator(c.name().as_slice()));
    (negated && is_and_or) || is_or || is_comparison
}

/// RuboCop's `left_hand_operand`.
fn left_hand_operand(ctx: &Context<'_>, condition: &Node<'_>, operator: &str) -> String {
    let text = String::from_utf8_lossy(ctx.text(condition.span())).into_owned();
    if matches!(condition, Node::OrNode { .. }) && operator == "&&" {
        format!("({text})")
    } else {
        text
    }
}

/// RuboCop's `add_parentheses_to_method_arguments`.
fn add_parentheses_to_method_arguments(ctx: &Context<'_>, call: &CallNode<'_>) -> String {
    let mut expr = String::new();
    if let Some(receiver) = call.receiver() {
        expr.push_str(&String::from_utf8_lossy(ctx.text(receiver.span())));
        expr.push('.');
    }
    expr.push_str(&String::from_utf8_lossy(call.name().as_slice()));
    expr.push('(');
    if let Some(arguments) = call.arguments() {
        let parts: Vec<String> = arguments
            .arguments()
            .iter()
            .map(|arg| String::from_utf8_lossy(ctx.text(arg.span())).into_owned())
            .collect();
        expr.push_str(&parts.join(", "));
    }
    expr.push(')');
    expr
}

/// RuboCop's `right_hand_operand`.
fn right_hand_operand(ctx: &Context<'_>, inner: &Modifier<'_>, left_hand_keyword: &str) -> String {
    let condition = &inner.condition;
    let negated = left_hand_keyword != inner.keyword;

    let mut expr = match as_plain_call(condition) {
        Some(call)
            if call.arguments().is_some_and(|a| !a.arguments().is_empty())
                && !is_operator_method(call.name().as_slice()) =>
        {
            add_parentheses_to_method_arguments(ctx, &call)
        }
        _ => String::from_utf8_lossy(ctx.text(condition.span())).into_owned(),
    };
    if requires_parens(condition, negated) {
        expr = format!("({expr})");
    }
    if negated {
        expr = format!("!{expr}");
    }
    expr
}

/// RuboCop's `new_expression` plus `replacement_operator`.
fn new_expression(ctx: &Context<'_>, outer: &Modifier<'_>, inner: &Modifier<'_>) -> String {
    let operator = if outer.keyword == "if" { "&&" } else { "||" };
    let lh_operand = left_hand_operand(ctx, &outer.condition, operator);
    let rh_operand = right_hand_operand(ctx, inner, outer.keyword);
    format!("{} {lh_operand} {operator} {rh_operand}", outer.keyword)
}

/// Checks for nested use of if, unless, while and until in their modifier
/// form.
#[derive(Debug, Clone)]
pub struct NestedModifier {
    /// Spans of nodes already reported this file (RuboCop's `ignore_node`):
    /// a node fully nested inside one of these is `part_of_ignored_node?`
    /// and is skipped, since it was already used as the inner modifier of
    /// an outer offense.
    ignored: Vec<Span>,
}

impl Rule for NestedModifier {
    const META: RuleMeta = RuleMeta {
        name: "Style/NestedModifier",
        department: Department::Style,
        summary: "Avoid using nested modifiers.",
        explanation: "\
Checks for nested use of if, unless, while and until in their modifier form.

```ruby
# bad
something if a if b

# good
something if b && a
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode, NodeKind::WhileNode, NodeKind::UntilNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { ignored: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.is_ignored(node.span()) {
            return;
        }
        let Some(outer) = modifier_shape(node) else { return };
        let Some(inner) = modifier_shape(&outer.body) else { return };

        if inner.is_if_family && outer.is_if_family {
            let range = Span::new(inner.keyword_span.start, outer.condition.span().end);
            let replacement = new_expression(ctx, &outer, &inner);
            let fix = Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, replacement.into_bytes())],
            };
            ctx.report_with_fix(&Self::META, inner.keyword_span, MSG, fix);
        } else {
            ctx.report(&Self::META, inner.keyword_span, MSG);
        }
        self.ignored.push(outer.body.span());
    }
}

impl NestedModifier {
    /// RuboCop's `part_of_ignored_node?`.
    fn is_ignored(&self, span: Span) -> bool {
        self.ignored.iter().any(|s| s.start <= span.start && span.end <= s.end)
    }
}
