//! `Minitest/SkipWithoutReason`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/skip_without_reason.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::{CallNode, CaseNode, IfNode, StatementsNode, UnlessNode};
use ruby_ast::{walk, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

const MSG: &str = "Add a reason explaining why the test is skipped.";

/// Checks for skipped tests missing the skipping reason.
#[derive(Debug, Clone)]
pub struct SkipWithoutReason;

impl Rule for SkipWithoutReason {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/SkipWithoutReason",
        department: Department::Minitest,
        summary: "Checks for skipped tests missing the skipping reason.",
        explanation: "Checks for skipped tests missing the skipping reason.\n\n```ruby\n# bad\nskip\nskip('')\n\n# bad\nif condition?\n  skip\nelse\n  skip\nend\n\n# good\nskip(\"Reason why the test was skipped\")\n\n# good\nskip if condition?\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ProgramNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let mut visitor = Skips { stack: Vec::new(), offenses: Vec::new() };
        walk(node, &mut visitor);
        for span in visitor.offenses {
            ctx.report(&Self::META, span, MSG);
        }
    }
}

struct Skips<'pr> {
    stack: Vec<Node<'pr>>,
    offenses: Vec<Span>,
}

impl<'pr> Visitor<'pr> for Skips<'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        if let Some(call) = node.as_call_node() {
            if call.name().as_slice() == b"skip" && self.offense(&call, node) {
                self.offenses.push(node.span());
            }
        }
        self.stack.push(*node);
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.stack.pop();
    }
}

impl<'pr> Skips<'pr> {
    /// The body of `on_send`.
    fn offense(&self, call: &CallNode<'pr>, node: &Node<'pr>) -> bool {
        if call.receiver().is_some() || !blank_argument(call) {
            return false;
        }
        let (parent, resbody) = self.parent(node);
        if let Some(conditional) = parent.and_then(|index| self.conditional_parent(index)) {
            if !only_skip_branches(&conditional) {
                return false;
            }
        }
        !resbody
    }

    /// The whitequark parent of `node`: an index into the ancestors (`None`
    /// for the root), and whether it is a `resbody`.
    fn parent(&self, node: &Node<'pr>) -> (Option<usize>, bool) {
        let count = self.stack.len();
        let Some(last) = self.stack.last() else { return (None, false) };
        if let Some(statements) = last.as_statements_node() {
            if statements.body().len() > 1 {
                return (None, false);
            }
            // A lone statement has no `begin` of its own, and `else` is not a
            // node of its own.
            let Some(mut index) = count.checked_sub(2) else { return (None, false) };
            if self.stack[index].as_else_node().is_some() {
                let Some(above) = index.checked_sub(1) else { return (None, false) };
                index = above;
            }
            if self.stack[index].as_program_node().is_some() {
                return (None, false);
            }
            (Some(index), self.stack[index].as_rescue_node().is_some())
        } else if last.as_arguments_node().is_some() {
            (count.checked_sub(2), false)
        } else if let Some(modifier) = last.as_rescue_modifier_node() {
            let span = modifier.rescue_expression().span();
            let node_span = node.span();
            if span.start == node_span.start && span.end == node_span.end {
                (None, true)
            } else {
                (Some(count - 1), false)
            }
        } else {
            (Some(count - 1), self.stack[count - 1].as_rescue_node().is_some())
        }
    }

    /// `conditional_parent`
    fn conditional_parent(&self, index: usize) -> Option<Node<'pr>> {
        let parent = self.stack[index];
        if parent.as_if_node().is_some()
            || parent.as_unless_node().is_some()
            || parent.as_case_node().is_some()
        {
            Some(parent)
        } else if parent.as_when_node().is_some() {
            index.checked_sub(1).map(|above| self.stack[above])
        } else {
            None
        }
    }
}

/// `blank_argument?`
fn blank_argument(call: &CallNode<'_>) -> bool {
    let first = call.arguments().and_then(|arguments| arguments.arguments().iter().next());
    match first {
        None => call.block().is_none_or(|block| block.as_block_argument_node().is_none()),
        Some(message) => {
            message.as_string_node().is_some_and(|string| string.unescaped().is_empty())
        }
    }
}

/// `only_skip_branches?`
fn only_skip_branches(node: &Node<'_>) -> bool {
    let branches = branches(node);
    branches.len() > 1
        && branches.iter().all(|branch| {
            branch.as_call_node().is_some_and(|call| {
                call.name().as_slice() == b"skip"
                    && !call.is_safe_navigation()
                    && call.block().is_none_or(|block| block.as_block_node().is_none())
            })
        })
}

/// `branches.compact` of an `if`, `unless` or `case`.
fn branches<'pr>(node: &Node<'pr>) -> Vec<Node<'pr>> {
    let mut out = Vec::new();
    if let Some(if_node) = node.as_if_node() {
        if_branches(&if_node, &mut out);
    } else if let Some(unless) = node.as_unless_node() {
        unless_branches(&unless, &mut out);
    } else if let Some(case) = node.as_case_node() {
        case_branches(&case, &mut out);
    }
    out
}

fn if_branches<'pr>(node: &IfNode<'pr>, out: &mut Vec<Node<'pr>>) {
    out.extend(body(node.statements()));
    if let Some(subsequent) = node.subsequent() {
        if let Some(elsif) = subsequent.as_if_node() {
            if_branches(&elsif, out);
        } else if let Some(else_node) = subsequent.as_else_node() {
            out.extend(body(else_node.statements()));
        }
    }
}

fn unless_branches<'pr>(node: &UnlessNode<'pr>, out: &mut Vec<Node<'pr>>) {
    out.extend(body(node.statements()));
    if let Some(else_node) = node.else_clause() {
        out.extend(body(else_node.statements()));
    }
}

fn case_branches<'pr>(node: &CaseNode<'pr>, out: &mut Vec<Node<'pr>>) {
    for when in &node.conditions() {
        if let Some(when) = when.as_when_node() {
            out.extend(body(when.statements()));
        }
    }
    if let Some(else_node) = node.else_clause() {
        out.extend(body(else_node.statements()));
    }
}

/// A branch's body: the lone statement, or the implicit `begin`.
fn body(statements: Option<StatementsNode<'_>>) -> Option<Node<'_>> {
    let statements = statements?;
    let list = statements.body();
    if list.len() == 1 {
        list.iter().next()
    } else {
        Some(statements.as_node())
    }
}
