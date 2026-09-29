//! `Lint/AssignmentInCondition`, ported from RuboCop's
//! `lib/rubocop/cop/lint/assignment_in_condition.rb` plus the `SafeAssignment`
//! mixin it includes.
//!
//! Whitequark's single `:if` node type covers `if`, `unless`, ternaries and
//! modifier forms; Prism splits it into distinct `IfNode`/`UnlessNode` kinds
//! (ternary = `IfNode` with no `if_keyword_loc`). Upstream's single `on_if`
//! (aliased to `on_while`/`on_until`) therefore becomes four subscribed
//! kinds here, each contributing its own `predicate()` as the condition to
//! walk; every node is still visited exactly once.
//!
//! Whitequark's `begin` node (any parenthesized/grouped expression) is
//! Prism's `ParenthesesNode`; its wrapped statement(s) always sit inside a
//! `StatementsNode`, even for a single statement (unlike whitequark, which
//! elides the wrapper when there is only one). `condition.children`/
//! `children.size` checks below go through `ParenthesesNode::body` ->
//! `StatementsNode::body`.
//!
//! `send`/`csend` (`ASGN_TYPES`'s `:send`/`:csend`) both map to Prism's
//! `CallNode`; `assignment_method?`/`setter_method?` (whitequark's
//! `loc.operator` presence) is `CallNode::equal_loc().is_some()`, matching
//! the approximation used elsewhere in this crate (e.g.
//! `Style/TernaryParentheses`, `Style/SoleNestedConditional`). A `CallNode`
//! carrying a literal block (`any_errors? { o = inspect(file) }`) is never
//! an attribute write, so it is always pruned by the plain "call but not an
//! assignment method" check below before its block body would ever be
//! reached -- no separate `any_block_type?` guard is needed.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::ParenthesesNode;
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_WITH_SAFE_ASSIGNMENT_ALLOWED: &str = "Use `==` if you meant to do a comparison or wrap \
the expression in parentheses to indicate you meant to assign in a condition.";
const MSG_WITHOUT_SAFE_ASSIGNMENT_ALLOWED: &str =
    "Use `==` if you meant to do a comparison or move the assignment up out of the condition.";

/// Checks for assignments in the conditions of if/while/until.
#[derive(Debug, Clone)]
pub struct AssignmentInCondition {
    allow_safe_assignment: bool,
}

impl Rule for AssignmentInCondition {
    const META: RuleMeta = RuleMeta {
        name: "Lint/AssignmentInCondition",
        department: Department::Lint,
        summary: "Don't use assignment in conditions.",
        explanation: "`AllowSafeAssignment` option for safe assignment. By safe assignment we \
mean putting parentheses around an assignment to indicate \"I know I'm using an assignment as a \
condition. It's not a mistake.\"\n\nThis cop's autocorrection is unsafe because it assumes that \
the author meant to use an assignment result as a condition.",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode, NodeKind::WhileNode, NodeKind::UntilNode],
        config: &[ConfigOption {
            name: "AllowSafeAssignment",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Whether an assignment wrapped in parentheses (e.g. `if (test = 10)`) is \
allowed.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_safe_assignment: options.bool("AllowSafeAssignment") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let condition = match node.kind() {
            NodeKind::IfNode => node.as_if_node().map(|n| n.predicate()),
            NodeKind::UnlessNode => node.as_unless_node().map(|n| n.predicate()),
            NodeKind::WhileNode => node.as_while_node().map(|n| n.predicate()),
            NodeKind::UntilNode => node.as_until_node().map(|n| n.predicate()),
            _ => None,
        };
        let Some(condition) = condition else { return };
        self.traverse(&condition, None, ctx);
    }
}

impl AssignmentInCondition {
    /// RuboCop's `traverse_node` plus the `on_if` block: walks `node`'s
    /// subtree (an assignment `defined?` never executes, and a `CallNode`
    /// that isn't an assignment method is pruned instead of recursed into),
    /// reporting every assignment used as (part of) the condition.
    fn traverse(&self, node: &Node<'_>, parent: Option<&Node<'_>>, ctx: &mut Context<'_>) {
        if node.kind() == NodeKind::DefinedNode {
            return;
        }

        if is_asgn_type(node) {
            if self.skip_children(node) {
                return;
            }
            if !allowed_construct(node, parent) {
                self.report(node, ctx);
            }
        }

        // Prism wraps a `ParenthesesNode`'s statement(s) in an intervening
        // `StatementsNode`, unlike whitequark's `begin`, whose children are
        // the statements directly; skip that layer so `discarded_assignment?`
        // sees the `ParenthesesNode` as the immediate parent.
        let next_parent = if node.kind() == NodeKind::StatementsNode { parent } else { Some(node) };
        for_each_child(node, |child| self.traverse(child, next_parent, ctx));
    }

    /// RuboCop's `SafeAssignment#skip_children?`.
    fn skip_children(&self, node: &Node<'_>) -> bool {
        if let Some(call) = node.as_call_node() {
            if call.equal_loc().is_none() {
                return true;
            }
        }
        is_empty_condition(node) || (self.allow_safe_assignment && is_safe_assignment(node))
    }

    /// RuboCop's `on_if`'s `add_offense` block.
    fn report(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(op_span) = operator_span(node) else { return };
        let msg = if self.allow_safe_assignment {
            MSG_WITH_SAFE_ASSIGNMENT_ALLOWED
        } else {
            MSG_WITHOUT_SAFE_ASSIGNMENT_ALLOWED
        };
        if self.allow_safe_assignment {
            let span = node.span();
            ctx.report_with_fix(
                &Self::META,
                op_span,
                msg,
                Fix {
                    applicability: Applicability::Unsafe,
                    edits: vec![
                        Edit::insert(span.start, b"(".to_vec()),
                        Edit::insert(span.end, b")".to_vec()),
                    ],
                },
            );
        } else {
            ctx.report(&Self::META, op_span, msg);
        }
    }
}

/// RuboCop's `AssignmentInCondition::ASGN_TYPES`.
fn is_asgn_type(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::ParenthesesNode
            | NodeKind::LocalVariableWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::MultiWriteNode
            | NodeKind::CallNode
    )
}

/// RuboCop's `allowed_construct?`.
fn allowed_construct(node: &Node<'_>, parent: Option<&Node<'_>>) -> bool {
    if node.kind() == NodeKind::ParenthesesNode {
        return true;
    }
    operator_span(node).is_none() || is_discarded_assignment(parent)
}

/// RuboCop's `discarded_assignment?`: the node is a statement of a
/// multi-statement `begin` (Prism: its parent is a `ParenthesesNode` wrapping
/// more than one statement), so its value is never used as the condition.
fn is_discarded_assignment(parent: Option<&Node<'_>>) -> bool {
    let Some(paren) = parent.and_then(Node::as_parentheses_node) else { return false };
    paren_children(&paren).len() > 1
}

/// RuboCop's `SafeAssignment#empty_condition?`: `(begin)`, i.e. `()`.
fn is_empty_condition(node: &Node<'_>) -> bool {
    node.as_parentheses_node().is_some_and(|p| p.body().is_none())
}

/// RuboCop's `SafeAssignment#safe_assignment?`: `(begin {equals_asgn?
/// #setter_method?})` -- a parenthesized condition whose single child is an
/// assignment (`lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`/`casgn`/`masgn`, i.e. not
/// an `op_asgn`/`or_asgn`/`and_asgn` shorthand) or a setter-method call
/// (`foo.bar = baz`, recognised in Prism by `CallNode::equal_loc`).
fn is_safe_assignment(node: &Node<'_>) -> bool {
    let Some(paren) = node.as_parentheses_node() else { return false };
    let children = paren_children(&paren);
    let [child] = children.as_slice() else { return false };
    is_equals_asgn(child) || child.as_call_node().is_some_and(|c| c.equal_loc().is_some())
}

fn is_equals_asgn(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::LocalVariableWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::MultiWriteNode
    )
}

/// The direct children of a parenthesized condition -- RuboCop's
/// `condition.to_a`/`condition.children` on a `begin` node.
fn paren_children<'pr>(paren: &ParenthesesNode<'pr>) -> Vec<Node<'pr>> {
    paren
        .body()
        .and_then(|body| body.as_statements_node())
        .map(|stmts| stmts.body().iter().collect())
        .unwrap_or_default()
}

/// RuboCop's `asgn_node.loc.operator`: `=` for the equals-assignment types
/// and `MultiWriteNode`, `CallNode::equal_loc` for a setter-method call.
fn operator_span(node: &Node<'_>) -> Option<Span> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => {
            node.as_local_variable_write_node().map(|n| n.operator_loc().span())
        }
        NodeKind::InstanceVariableWriteNode => {
            node.as_instance_variable_write_node().map(|n| n.operator_loc().span())
        }
        NodeKind::ClassVariableWriteNode => {
            node.as_class_variable_write_node().map(|n| n.operator_loc().span())
        }
        NodeKind::GlobalVariableWriteNode => {
            node.as_global_variable_write_node().map(|n| n.operator_loc().span())
        }
        NodeKind::ConstantWriteNode => {
            node.as_constant_write_node().map(|n| n.operator_loc().span())
        }
        NodeKind::ConstantPathWriteNode => {
            node.as_constant_path_write_node().map(|n| n.operator_loc().span())
        }
        NodeKind::MultiWriteNode => node.as_multi_write_node().map(|n| n.operator_loc().span()),
        NodeKind::CallNode => node.as_call_node().and_then(|c| c.equal_loc()).map(|l| l.span()),
        _ => None,
    }
}
