//! `Style/SwapValues`, ported from RuboCop's
//! `lib/rubocop/cop/style/swap_values.rb`.
//!
//! Upstream walks `node.right_siblings` of a whitequark `(l|i|c|g|c)vasgn`/
//! `casgn` node to find the next two statements. Prism has no sibling
//! accessor, so this port instead visits every [`NodeKind::StatementsNode`]
//! once and scans consecutive windows of its own `body` directly -- the same
//! triples upstream reaches one at a time via `on_asgn` plus
//! `right_siblings.take(2)`.
//!
//! Upstream's `allowed_assignment?` (`node.parent&.mlhs_type? ||
//! node.parent&.shorthand_asgn?`) excludes a simple-assignment node that is
//! actually a `masgn` target or an `op_asgn`/`or_asgn`/`and_asgn` mutation.
//! Neither shape exists for Prism's `*WriteNode` kinds: multi-assignment
//! targets are distinct `*TargetNode` kinds, and shorthand assignment is a
//! distinct `*OperatorWriteNode`/`*OrWriteNode`/`*AndWriteNode` kind -- so no
//! equivalent check is needed here.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::const_name;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str =
    "Replace this and assignments at lines {x_line} and {y_line} with `{replacement}`.";

/// Whether `node` is one of RuboCop's `SIMPLE_ASSIGNMENT_TYPES`.
fn is_simple_assignment(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::LocalVariableWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
    )
}

/// RuboCop's `lhs`: the assigned name's own source text (`"#{'::' if
/// node.absolute?}#{node.const_name}"` for a `casgn`, `node.name.to_s`
/// otherwise).
fn lhs(node: &Node<'_>) -> Option<String> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => Some(
            String::from_utf8_lossy(node.as_local_variable_write_node()?.name().as_slice())
                .into_owned(),
        ),
        NodeKind::InstanceVariableWriteNode => Some(
            String::from_utf8_lossy(node.as_instance_variable_write_node()?.name().as_slice())
                .into_owned(),
        ),
        NodeKind::ClassVariableWriteNode => Some(
            String::from_utf8_lossy(node.as_class_variable_write_node()?.name().as_slice())
                .into_owned(),
        ),
        NodeKind::GlobalVariableWriteNode => Some(
            String::from_utf8_lossy(node.as_global_variable_write_node()?.name().as_slice())
                .into_owned(),
        ),
        NodeKind::ConstantWriteNode => Some(
            String::from_utf8_lossy(node.as_constant_write_node()?.name().as_slice()).into_owned(),
        ),
        NodeKind::ConstantPathWriteNode => {
            let w = node.as_constant_path_write_node()?;
            let target = w.target();
            let target_node = target.as_node();
            let name = const_name(&target_node)?;
            Some(if target.parent().is_none() { format!("::{name}") } else { name })
        }
        _ => None,
    }
}

/// RuboCop's `rhs`: `node.expression.source`, the assigned value's own
/// source text.
fn rhs(ctx: &Context<'_>, node: &Node<'_>) -> Option<String> {
    let value = match node.kind() {
        NodeKind::LocalVariableWriteNode => node.as_local_variable_write_node()?.value(),
        NodeKind::InstanceVariableWriteNode => node.as_instance_variable_write_node()?.value(),
        NodeKind::ClassVariableWriteNode => node.as_class_variable_write_node()?.value(),
        NodeKind::GlobalVariableWriteNode => node.as_global_variable_write_node()?.value(),
        NodeKind::ConstantWriteNode => node.as_constant_write_node()?.value(),
        NodeKind::ConstantPathWriteNode => node.as_constant_path_write_node()?.value(),
        _ => return None,
    };
    Some(String::from_utf8_lossy(ctx.text(value.span())).into_owned())
}

/// Enforces the use of shorthand-style swapping of 2 variables.
#[derive(Debug, Clone)]
pub struct SwapValues;

impl Rule for SwapValues {
    const META: RuleMeta = RuleMeta {
        name: "Style/SwapValues",
        department: Department::Style,
        summary: "Enforces the use of shorthand-style swapping of 2 variables.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(statements) = node.as_statements_node() else { return };
        let body: Vec<Node<'_>> = statements.body().iter().collect();

        for window in body.windows(3) {
            let [tmp_assign, x_assign, y_assign] = window else { continue };
            if !is_simple_assignment(tmp_assign)
                || !is_simple_assignment(x_assign)
                || !is_simple_assignment(y_assign)
            {
                continue;
            }
            let (Some(lhs_x), Some(rhs_tmp)) = (lhs(x_assign), rhs(ctx, tmp_assign)) else {
                continue;
            };
            if lhs_x != rhs_tmp {
                continue;
            }
            let (Some(lhs_y), Some(rhs_x)) = (lhs(y_assign), rhs(ctx, x_assign)) else {
                continue;
            };
            if lhs_y != rhs_x {
                continue;
            }
            let (Some(lhs_tmp), Some(rhs_y)) = (lhs(tmp_assign), rhs(ctx, y_assign)) else {
                continue;
            };
            if rhs_y != lhs_tmp {
                continue;
            }

            let x_line = ctx.line_col(x_assign.span().start).line;
            let y_line = ctx.line_col(y_assign.span().start).line;
            let replacement = format!("{lhs_x}, {rhs_x} = {rhs_x}, {lhs_x}");
            let msg = MSG
                .replace("{x_line}", &x_line.to_string())
                .replace("{y_line}", &y_line.to_string())
                .replace("{replacement}", &replacement);

            let start_line = ctx.line_col(tmp_assign.span().start).line;
            let end_line = ctx
                .line_col(y_assign.span().end.saturating_sub(1).max(tmp_assign.span().start))
                .line;
            let range = ruby_source::Span::new(
                ctx.line_span(start_line).start,
                ctx.line_span(end_line).end,
            );
            let fix = Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, replacement.into_bytes())],
            };
            ctx.report_with_fix(&Self::META, tmp_assign.span(), msg, fix);
        }
    }
}
