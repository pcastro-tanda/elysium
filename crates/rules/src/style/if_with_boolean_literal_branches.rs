//! `Style/IfWithBooleanLiteralBranches`, ported from RuboCop's
//! `lib/rubocop/cop/style/if_with_boolean_literal_branches.rb`.
//!
//! whitequark's single `:if` node (ternary, modifier, plain `if`, `unless`,
//! and -- recursively through `else_branch` -- `elsif`) splits across
//! Prism's `IfNode`/`UnlessNode`: a ternary has no `if_keyword_loc`, a
//! modifier form has no `end_keyword_loc`, and an `elsif` link is reached
//! through `IfNode::subsequent` (`Some(Node::IfNode)`, whose own
//! `if_keyword_loc` reads `"elsif"`). Unlike `Style/MinMaxComparison`,
//! upstream never swaps `if_branch`/`else_branch` for `node.unless?` here,
//! so [`Shape::of`] reads `UnlessNode::statements`/`else_clause` in the same
//! first-/second-written order as `IfNode::statements`/`subsequent`'s
//! `ElseNode`, with no swap.
//!
//! An `elsif` link's own span, unlike whitequark's, extends through the
//! chain's *shared* final `end` (every link gets the same
//! `end_keyword_loc`); since this cop only ever rewrites an `elsif` whose
//! `else_branch` is a literal (ruled out by [`Shape::of`] otherwise, as a
//! further nested `elsif` can never itself be `true`/`false`), the
//! whitequark-equivalent span stops at that literal's own end instead of
//! `node.span().end` -- see [`elsif_replace_span`].

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Remove redundant %<keyword>s with boolean literal branches.";
/// RuboCop's `MSG_FOR_ELSIF`.
const MSG_FOR_ELSIF: &str =
    "Use `else` instead of redundant `elsif` with boolean literal branches.";

/// Checks for redundant `if` with boolean literal branches.
#[derive(Debug, Clone)]
pub struct IfWithBooleanLiteralBranches {
    allowed_methods: Vec<String>,
}

impl Rule for IfWithBooleanLiteralBranches {
    const META: RuleMeta = RuleMeta {
        name: "Style/IfWithBooleanLiteralBranches",
        department: Department::Style,
        summary: "Checks for redundant `if` with boolean literal branches.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[linter::ConfigOption {
            name: "AllowedMethods",
            default: linter::ConfigDefault::StrList(&["infinite?", "nonzero?"]),
            allowed: &[],
            doc: "Allowed predicate/comparison methods.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allowed_methods: options.str_list("AllowedMethods") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(shape) = Shape::of(node) else { return };
        if shape.is_elsif && Shape::parent_is_elsif(ctx) {
            return;
        }

        let condition = shape.predicate;
        if !self.return_boolean_value(condition) {
            return;
        }
        let Some(if_branch) = shape.first else { return };
        let Some(else_branch) = shape.second else { return };
        let if_true = if_branch.as_true_node().is_some();
        let if_false = if_branch.as_false_node().is_some();
        let else_true = else_branch.as_true_node().is_some();
        let else_false = else_branch.as_false_node().is_some();
        if !((if_true && else_false) || (if_false && else_true)) {
            return;
        }

        let opposite = if shape.is_unless { if_true } else { if_false };
        let bang = if opposite { "!" } else { "" };
        let condition_src = String::from_utf8_lossy(ctx.text(condition.span()));
        let replacement = if opposite && requires_parentheses(&condition) {
            format!("{bang}({condition_src})")
        } else {
            format!("{bang}{condition_src}")
        };

        let (range, keyword) = if shape.is_ternary {
            (Span::new(condition.span().end, node.span().end), "ternary operator".to_string())
        } else {
            let keyword_loc = shape.keyword_loc.expect("non-ternary has a keyword");
            (keyword_loc.span(), format!("`{}`", String::from_utf8_lossy(keyword_loc.as_slice())))
        };

        let message = if shape.is_elsif {
            MSG_FOR_ELSIF.to_string()
        } else {
            MSG.replace("%<keyword>s", &keyword)
        };

        let edits = if shape.is_elsif {
            let indent = line_indent(ctx, if_branch.span());
            let start = node.span().start;
            let end = else_branch.span().end;
            vec![
                Edit::insert(start, b"else\n".to_vec()),
                Edit::replace(Span::new(start, end), format!("{indent}{replacement}").into_bytes()),
            ]
        } else {
            vec![Edit::replace(node.span(), replacement.into_bytes())]
        };

        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

impl IfWithBooleanLiteralBranches {
    /// RuboCop's `return_boolean_value?`.
    fn return_boolean_value(&self, condition: Node<'_>) -> bool {
        let condition = unwrap_parens(condition);
        if let Some(or) = condition.as_or_node() {
            self.return_boolean_value(or.left()) && self.return_boolean_value(or.right())
        } else if let Some(and) = condition.as_and_node() {
            self.return_boolean_value(and.right())
        } else {
            self.assume_boolean_value(&condition)
        }
    }

    /// RuboCop's `assume_boolean_value?`: `condition.send_type?` is false
    /// for a call with an attached block -- whitequark wraps that in a
    /// distinct `block`/`numblock` node type around the `send`, where
    /// Prism keeps it as the same `CallNode` with a non-`nil` `block()` --
    /// so a block-attached call (`foo.any? { ... }`) is never treated as
    /// "assumed boolean" even though its own method name alone would
    /// otherwise qualify.
    fn assume_boolean_value(&self, condition: &Node<'_>) -> bool {
        let Some(call) = condition.as_call_node() else { return false };
        if call.block().and_then(|b| b.as_block_node()).is_some() {
            return false;
        }
        let name = call.name();
        let name = name.as_slice();
        if self.allowed_methods.iter().any(|m| m.as_bytes() == name) {
            return false;
        }
        is_comparison_method(name) || name.ends_with(b"?") || is_double_negative(&call)
    }
}

/// `rubocop-ast`'s `COMPARISON_OPERATORS`.
fn is_comparison_method(name: &[u8]) -> bool {
    matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=" | b">" | b"<")
}

/// RuboCop's `double_negative?`: `(send (send _ :!) :!)`.
fn is_double_negative(call: &ruby_ast::node::CallNode<'_>) -> bool {
    if call.name().as_slice() != b"!" {
        return false;
    }
    call.receiver()
        .and_then(|r| r.as_call_node())
        .is_some_and(|inner| inner.name().as_slice() == b"!")
}

/// RuboCop's `require_parentheses?`: `operator_keyword?` (an `and`/`or`
/// node, `&&`/`||` included -- whitequark's type check does not
/// distinguish the keyword spelling) or a comparison-method call.
fn requires_parentheses(condition: &Node<'_>) -> bool {
    condition.as_and_node().is_some()
        || condition.as_or_node().is_some()
        || condition.as_call_node().is_some_and(|call| is_comparison_method(call.name().as_slice()))
}

/// RuboCop's `(begin ...)` alternative: an explicitly parenthesized
/// expression, unwrapped to its last (sole, for a condition) statement.
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

/// The whitespace preceding `span`'s own line, RuboCop's `Util.indent`.
fn line_indent(ctx: &Context<'_>, span: Span) -> String {
    let line = ctx.line_col(span.start).line;
    let line_start = ctx.line_span(line).start;
    String::from_utf8_lossy(ctx.text(Span::new(line_start, span.start))).into_owned()
}

/// Unified view over `IfNode`/`UnlessNode`.
struct Shape<'pr> {
    predicate: Node<'pr>,
    first: Option<Node<'pr>>,
    second: Option<Node<'pr>>,
    is_unless: bool,
    is_ternary: bool,
    is_elsif: bool,
    keyword_loc: Option<ruby_ast::Location<'pr>>,
}

impl<'pr> Shape<'pr> {
    fn of(node: &Node<'pr>) -> Option<Self> {
        fn single_statement(node: Option<ruby_ast::node::StatementsNode<'_>>) -> Option<Node<'_>> {
            let statements = node?;
            if statements.body().len() == 1 {
                statements.body().first()
            } else {
                None
            }
        }

        if let Some(if_node) = node.as_if_node() {
            let second = match if_node.subsequent() {
                Some(Node::ElseNode { .. }) => single_statement(
                    if_node.subsequent().and_then(|n| n.as_else_node()?.statements()),
                ),
                _ => None,
            };
            let is_elsif = if_node.if_keyword_loc().is_some_and(|loc| loc.as_slice() == b"elsif");
            Some(Self {
                predicate: if_node.predicate(),
                first: single_statement(if_node.statements()),
                second,
                is_unless: false,
                is_ternary: if_node.if_keyword_loc().is_none(),
                is_elsif,
                keyword_loc: if_node.if_keyword_loc(),
            })
        } else if let Some(unless_node) = node.as_unless_node() {
            let second = single_statement(unless_node.else_clause().and_then(|e| e.statements()));
            Some(Self {
                predicate: unless_node.predicate(),
                first: single_statement(unless_node.statements()),
                second,
                is_unless: true,
                is_ternary: false,
                is_elsif: false,
                keyword_loc: Some(unless_node.keyword_loc()),
            })
        } else {
            None
        }
    }

    /// RuboCop's `multiple_elsif?`: the parent is itself an `elsif` link.
    /// Only a `(span, kind)` pair is available for an ancestor, not its
    /// typed accessors, so this sniffs the parent's own leading bytes --
    /// an `elsif` `IfNode`'s span starts exactly at its own
    /// `if_keyword_loc` (see the module docs).
    fn parent_is_elsif(ctx: &Context<'_>) -> bool {
        ctx.parent().is_some_and(|parent| {
            parent.kind == NodeKind::IfNode && ctx.text(parent.span).starts_with(b"elsif")
        })
    }
}
