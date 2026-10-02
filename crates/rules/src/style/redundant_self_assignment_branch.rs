//! `Style/RedundantSelfAssignmentBranch`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_self_assignment_branch.rb`.
//!
//! Whitequark's parser unifies `if`/`unless`/ternary into one `:if` node,
//! normalizing `unless`'s swapped branches via `IfNode#node_parts`; Prism
//! keeps `unless` as its own [`ruby_ast::node::UnlessNode`] and only models
//! `if`/ternary/`elsif` as [`ruby_ast::node::IfNode`] (ternary = no
//! `if_keyword_loc`, `elsif` = reached through `subsequent()`, a trailing
//! `else` = [`ruby_ast::node::ElseNode`] through the same). [`branches_of`]
//! reproduces that normalization so `if_branch` always means "runs when the
//! condition, as literally written, is truthy" regardless of keyword.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::StatementsNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Remove the self-assignment branch.";

/// Checks for places where conditional branch makes redundant self-assignment.
#[derive(Debug, Clone)]
pub struct RedundantSelfAssignmentBranch;

impl Rule for RedundantSelfAssignmentBranch {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantSelfAssignmentBranch",
        department: Department::Style,
        summary: "Checks for places where conditional branch makes redundant self-assignment.",
        explanation: "It only detects local variable because it may replace state of instance \
            variable, class variable, and global variable that have state across methods with \
            `nil`.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::LocalVariableWriteNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(lvasgn) = node.as_local_variable_write_node() else { return };
        let value = lvasgn.value();
        let Some((condition, if_branch, else_branch, elsif_chain)) = branches_of(&value) else {
            return;
        };
        if is_multi(&if_branch) || is_multi(&else_branch) || elsif_chain {
            return;
        }

        let name = lvasgn.name().as_slice();
        if self_assign(ctx, name, &if_branch) {
            register_offense(ctx, &value, &condition, &if_branch, &else_branch, "unless");
        } else if self_assign(ctx, name, &else_branch) {
            register_offense(ctx, &value, &condition, &else_branch, &if_branch, "if");
        }
    }
}

/// A normalized `if`/`unless` branch body: `rubocop-ast`'s `If#if_branch`/
/// `#else_branch`, which are `nil` for an empty branch, the lone statement
/// for a single-statement branch (whatever its own node kind), or the
/// synthetic `begin` wrapper's children for multiple statements.
enum Branch<'pr> {
    None,
    Single(Node<'pr>),
    Multi,
}

fn branch_of(stmts: Option<StatementsNode<'_>>) -> Branch<'_> {
    let Some(stmts) = stmts else { return Branch::None };
    let body = stmts.body();
    match body.len() {
        0 => Branch::None,
        1 => Branch::Single(body.first().expect("len == 1")),
        _ => Branch::Multi,
    }
}

/// RuboCop's `multiple_statements?`: true for a genuinely multi-statement
/// branch, but also -- because whitequark represents an explicitly
/// parenthesized expression as the same `begin` node type as a
/// multi-statement sequence, and the check only looks at `begin_type?` plus
/// non-empty children -- for a branch that is `(expr)` with at least one
/// statement inside (an *empty* `()` is a `begin` node with zero children,
/// so it is not treated as multi). Prism's
/// [`ruby_ast::node::ParenthesesNode`] is the equivalent single-statement
/// wrapper, so a `Branch::Single` holding a non-empty one is excluded the
/// same way.
fn is_multi(branch: &Branch<'_>) -> bool {
    match branch {
        Branch::Multi => true,
        Branch::Single(n) => n.as_parentheses_node().is_some_and(|p| {
            p.body().and_then(|b| b.as_statements_node()).is_some_and(|s| !s.body().is_empty())
        }),
        Branch::None => false,
    }
}

/// `rubocop-ast`'s normalized `node_parts`: `(condition, if_branch,
/// else_branch)`, plus whether `else_branch` is itself an `elsif`
/// continuation (RuboCop's `else_branch.respond_to?(:elsif?) &&
/// else_branch.elsif?`). `None` if `value` is not an `if`/`unless`
/// expression at all.
fn branches_of<'pr>(value: &Node<'pr>) -> Option<(Node<'pr>, Branch<'pr>, Branch<'pr>, bool)> {
    match value {
        Node::IfNode { .. } => {
            let n = value.as_if_node().expect("kind matched");
            let if_branch = branch_of(n.statements());
            let (else_branch, elsif_chain) = match n.subsequent() {
                None => (Branch::None, false),
                Some(sub) => match &sub {
                    Node::IfNode { .. } => (Branch::Single(sub), true),
                    Node::ElseNode { .. } => {
                        let e = sub.as_else_node().expect("subsequent is if or else");
                        (branch_of(e.statements()), false)
                    }
                    _ => unreachable!("IfNode#subsequent is an if or an else"),
                },
            };
            Some((n.predicate(), if_branch, else_branch, elsif_chain))
        }
        Node::UnlessNode { .. } => {
            let n = value.as_unless_node().expect("kind matched");
            let if_branch = match n.else_clause() {
                None => Branch::None,
                Some(e) => branch_of(e.statements()),
            };
            let else_branch = branch_of(n.statements());
            Some((n.predicate(), if_branch, else_branch, false))
        }
        _ => None,
    }
}

/// RuboCop's `self_assign?`: `variable.to_s == branch&.source`.
fn self_assign(ctx: &Context<'_>, name: &[u8], branch: &Branch<'_>) -> bool {
    match branch {
        Branch::Single(n) => ctx.text(n.span()) == name,
        _ => false,
    }
}

/// RuboCop's `register_offense`.
fn register_offense(
    ctx: &mut Context<'_>,
    if_node: &Node<'_>,
    condition: &Node<'_>,
    offense_branch: &Branch<'_>,
    opposite_branch: &Branch<'_>,
    keyword: &str,
) {
    let Branch::Single(offense_node) = offense_branch else { return };
    let span = offense_node.span();

    let condition_text = String::from_utf8_lossy(ctx.text(condition.span())).into_owned();
    let replacement = match opposite_branch {
        Branch::Single(opposite_node) => {
            let assignment_value =
                String::from_utf8_lossy(ctx.text(opposite_node.span())).into_owned();
            let mut replacement = format!("{assignment_value} {keyword} {condition_text}");
            if ruby_ast::ext::is_heredoc(opposite_node) {
                let s = opposite_node.as_string_node().expect("heredoc is a StringNode");
                let closing = s.closing_loc().expect("heredoc has a closing").span();
                let end = if ctx.text(closing).last() == Some(&b'\n') {
                    closing.end - 1
                } else {
                    closing.end
                };
                let tail = Span::new(opposite_node.span().end, end);
                replacement.push_str(&String::from_utf8_lossy(ctx.text(tail)));
            }
            replacement
        }
        _ => format!("nil {keyword} {condition_text}"),
    };

    ctx.report_with_fix(
        &RedundantSelfAssignmentBranch::META,
        span,
        MSG,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(if_node.span(), replacement.into_bytes())],
        },
    );
}
