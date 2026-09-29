//! `Style/EmptyCaseCondition`, ported from RuboCop's
//! `lib/rubocop/cop/style/empty_case_condition.rb`.
//!
//! # `node.parent` across Prism's extra wrapper nodes
//!
//! Upstream's `NOT_SUPPORTED_PARENT_TYPES` check and the `when_node.parent.parent`
//! guard in `replace_then_with_line_break` both read whitequark's `node.parent`
//! directly. Prism has no equivalent of a bare `case` sitting as a direct child
//! of `return`/`break`/`next`/`yield`/`super`/a call: it always interposes an
//! `ArgumentsNode` between the keyword and its argument. It also always wraps a
//! statement sequence in a `StatementsNode`, even when it holds a single
//! statement that whitequark's `begin` elision would otherwise leave bare (the
//! sole top-level statement in a file has no parent at all upstream).
//! [`logical_parent`] bridges both gaps by skipping any such wrapper whose span
//! exactly matches the node it wraps, and reports `None` once the walk reaches
//! `ProgramNode` -- matching whitequark's "no parent" for a lone top-level
//! statement.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, NodeInfo, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CaseNode, WhenNode};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Do not use empty `case` condition, instead use an `if` expression.";

/// Avoid empty condition in case statements.
#[derive(Debug, Clone)]
pub struct EmptyCaseCondition;

impl Rule for EmptyCaseCondition {
    const META: RuleMeta = RuleMeta {
        name: "Style/EmptyCaseCondition",
        department: Department::Style,
        summary: "Avoid empty condition in case statements.",
        explanation: "\
```ruby
# bad:
case
when x == 0
  puts 'x is 0'
when y == 0
  puts 'y is 0'
else
  puts 'neither is 0'
end

# good:
if x == 0
  puts 'x is 0'
elsif y == 0
  puts 'y is 0'
else
  puts 'neither is 0'
end

# good: (the case condition node is not empty)
case n
when 0
  puts 'zero'
when 1
  puts 'one'
else
  puts 'more'
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CaseNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(case_node) = node.as_case_node() else { return };
        if case_node.predicate().is_some() {
            return;
        }

        let parent = logical_parent(ctx.ancestors(), case_node.as_node().span());
        if parent.is_some_and(|p| {
            matches!(
                p.kind,
                NodeKind::ReturnNode
                    | NodeKind::BreakNode
                    | NodeKind::NextNode
                    | NodeKind::CallNode
                    | NodeKind::YieldNode
                    | NodeKind::SuperNode
            )
        }) {
            return;
        }

        let when_nodes: Vec<WhenNode<'_>> =
            case_node.conditions().iter().filter_map(|n| n.as_when_node()).collect();
        let Some(first_when) = when_nodes.first() else { return };

        let branches_return = when_nodes
            .iter()
            .filter_map(WhenNode::statements)
            .chain(case_node.else_clause().and_then(|e| e.statements()))
            .any(|stmts| branch_has_return(&stmts.as_node()));
        if branches_return {
            return;
        }

        let span = case_node.case_keyword_loc().span();
        let fix = build_fix(ctx, &case_node, first_when, &when_nodes, parent.is_some());
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}

/// RuboCop's `Node#parent`, bridged across Prism's `ArgumentsNode` (absent
/// from whitequark, where a call's/keyword's arguments are its own direct
/// children) and a `StatementsNode` that holds only one statement (whitequark
/// elides a single-statement `begin`). Once the walk reaches `ProgramNode`,
/// whitequark has no equivalent node at all, so that is reported as no
/// parent (`None`).
fn logical_parent(ancestors: &[NodeInfo], node_span: Span) -> Option<NodeInfo> {
    let mut current_span = node_span;
    let mut idx = ancestors.len();
    loop {
        idx = idx.checked_sub(1)?;
        let info = ancestors[idx];
        let transparent = (info.kind == NodeKind::ArgumentsNode
            || info.kind == NodeKind::StatementsNode)
            && info.span == current_span;
        current_span = info.span;
        if transparent {
            continue;
        }
        return (info.kind != NodeKind::ProgramNode).then_some(info);
    }
}

/// RuboCop's `body.return_type? || body.each_descendant.any?(&:return_type?)`.
/// Prism always wraps a `when`/`else` body in a `StatementsNode` (never a bare
/// `return_type?` node), so the two upstream checks collapse into one
/// descendant walk from that wrapper.
fn branch_has_return(statements: &Node<'_>) -> bool {
    let mut found = false;
    each_descendant(statements, &mut |descendant| {
        if descendant.kind() == NodeKind::ReturnNode {
            found = true;
        }
    });
    found
}

/// RuboCop's `autocorrect`.
fn build_fix(
    ctx: &Context<'_>,
    case_node: &CaseNode<'_>,
    first_when: &WhenNode<'_>,
    when_nodes: &[WhenNode<'_>],
    has_parent: bool,
) -> Fix {
    let mut edits = Vec::new();

    let case_range =
        Span::new(case_node.case_keyword_loc().span().start, first_when.keyword_loc().span().end);
    edits.push(Edit::replace(case_range, b"if".to_vec()));
    if let Some(comments) = first_when_comments(ctx, case_range) {
        let line_start = ctx.line_span(ctx.line_col(case_range.start).line).start;
        edits.push(Edit::insert(line_start, comments.into_bytes()));
    }
    for when_node in &when_nodes[1..] {
        edits.push(Edit::replace(when_node.keyword_loc().span(), b"elsif".to_vec()));
    }

    for when_node in when_nodes {
        correct_when_conditions(ctx, when_node, has_parent, &mut edits);
    }

    Fix { applicability: Applicability::Safe, edits }
}

/// RuboCop's `keep_first_when_comment`: every comment on a line from
/// `case_range`'s first line up to (excluding) its last line, each reindented
/// to the `case` keyword's own column. `None` when there is nothing to move.
fn first_when_comments(ctx: &Context<'_>, case_range: Span) -> Option<String> {
    let indent = " ".repeat(ctx.line_col(case_range.start).column as usize);
    let first_line = ctx.line_col(case_range.start).line;
    let last_line = ctx.line_col(case_range.end).line;
    let mut text = String::new();
    for comment in ctx.comments() {
        if comment.line >= first_line && comment.line < last_line {
            text.push_str(&indent);
            text.push_str(&String::from_utf8_lossy(ctx.text(comment.span)));
            text.push('\n');
        }
    }
    (!text.is_empty()).then_some(text)
}

/// RuboCop's `correct_when_conditions`.
fn correct_when_conditions(
    ctx: &Context<'_>,
    when_node: &WhenNode<'_>,
    has_parent: bool,
    edits: &mut Vec<Edit>,
) {
    let conditions: Vec<Node<'_>> = when_node.conditions().iter().collect();
    let Some(last_condition) = conditions.last() else { return };

    if has_parent {
        if let Some(then_loc) = when_node.then_keyword_loc() {
            let range = Span::new(last_condition.span().end, then_loc.span().end);
            edits.push(Edit::replace(range, b"\n".to_vec()));
        }
    }

    if conditions.len() > 1 {
        let range = Span::new(conditions[0].span().start, last_condition.span().end);
        let joined = conditions
            .iter()
            .map(|c| parenthesize_condition(ctx, c))
            .collect::<Vec<_>>()
            .join(" || ");
        edits.push(Edit::replace(range, joined.into_bytes()));
    }
}

/// RuboCop's `parenthesize_condition`: a condition that binds looser than
/// `||` (a ternary/`unless` -- both whitequark's `:if` -- an `and`/`or`
/// keyword form, or a range) must be parenthesized so the joined `||` keeps
/// its meaning.
fn parenthesize_condition(ctx: &Context<'_>, condition: &Node<'_>) -> String {
    let source = String::from_utf8_lossy(ctx.text(condition.span()));
    if is_assignment(condition)
        || matches!(
            condition.kind(),
            NodeKind::IfNode
                | NodeKind::UnlessNode
                | NodeKind::AndNode
                | NodeKind::OrNode
                | NodeKind::RangeNode
        )
    {
        format!("({source})")
    } else {
        source.into_owned()
    }
}

/// RuboCop-AST's `Node::ASSIGNMENTS` (`EQUALS_ASSIGNMENTS` +
/// `SHORTHAND_ASSIGNMENTS`): every `lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`/
/// `casgn`/`masgn`/`op_asgn`/`or_asgn`/`and_asgn` shape, mapped onto Prism's
/// per-target-kind `*WriteNode` family.
fn is_assignment(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
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
