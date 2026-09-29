//! `Style/OrAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/style/or_assignment.rb`.
//!
//! Whitequark unifies `if`/`unless`/ternary into a single `:if` node type
//! (a literal/modifier `unless` is `(if COND nil BODY)`: the condition
//! swapped with the body, and the "then" position left `nil`), so upstream
//! covers all four surface forms -- `var = var ? var : x`,
//! `var = if var; var; else; x; end`, `var = x unless var`, and
//! `unless var; var = x; end` -- plus the coincidental "empty `then`
//! branch" case (`if var; else; var = x; end`, structurally identical to
//! `unless`) with just two `def_node_matcher`s (`ternary_assignment?`,
//! `unless_assignment?`) and two handlers (`on_lvasgn` et al., `on_if`).
//!
//! Prism keeps `IfNode` and `UnlessNode` as distinct kinds (a ternary is an
//! `IfNode` with no `if_keyword_loc` but otherwise the same shape:
//! `predicate`, then-`StatementsNode`, `subsequent`), so this port
//! dispatches on three kinds instead:
//!
//! - `Local/Instance/Class/GlobalVariableWriteNode` whose `value` is an
//!   `IfNode` (`ternary_assignment?`): covers the ternary and the
//!   `if var; var; else; x; end` forms uniformly.
//! - `IfNode` (`unless_assignment?` applied directly): the "empty `then`
//!   branch" case, where `statements` is `None` and `subsequent` is an
//!   `ElseNode` wrapping the assignment.
//! - `UnlessNode`: the literal/modifier `unless` forms, where `statements`
//!   holds the assignment and there is no `else_clause`.
//!
//! An `elsif` chain (`subsequent` resolving to another `IfNode` rather than
//! an `ElseNode`) is excluded from the first two by construction, matching
//! upstream's `return if else_branch.if_type?` guard.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::StatementsNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Use the double pipe equals operator `||=` instead.";

/// Recommend usage of double pipe equals (||=) where applicable.
#[derive(Debug, Clone)]
pub struct OrAssignment;

impl Rule for OrAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Style/OrAssignment",
        department: Department::Style,
        summary: "Recommend usage of double pipe equals (||=) where applicable.",
        explanation: "\
Checks for potential usage of the `||=` operator.

```ruby
# bad
name = name ? name : 'Bozhidar'

# bad
name = if name
         name
       else
         'Bozhidar'
       end

# bad
unless name
  name = 'Bozhidar'
end

# bad
name = 'Bozhidar' unless name

# good - set name to 'Bozhidar', only if it's nil or false
name ||= 'Bozhidar'
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::LocalVariableWriteNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::ClassVariableWriteNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::IfNode,
            NodeKind::UnlessNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::LocalVariableWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::GlobalVariableWriteNode => Self::check_assignment(node, ctx),
            NodeKind::IfNode => Self::check_empty_then_if(node, ctx),
            NodeKind::UnlessNode => Self::check_unless(node, ctx),
            _ => {}
        }
    }
}

impl OrAssignment {
    /// RuboCop's `on_lvasgn`/`on_ivasgn`/`on_cvasgn`/`on_gvasgn`, applying
    /// `ternary_assignment?` (`take_variable_and_default_from_ternary`
    /// folded in: `variable` is the assignment's own name, `default` is the
    /// `else`-branch source).
    fn check_assignment(node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some((name, value)) = assignment_name_value(node) else { return };
        let Some(default_span) = ternary_default_span(name, &value) else { return };
        let default_src = ctx.text(default_span).to_vec();
        Self::report(ctx, node.span(), name, &default_src);
    }

    /// RuboCop's `on_if`, applying `unless_assignment?` for the "empty
    /// `then` branch" case only -- a genuine `unless` is Prism's
    /// `UnlessNode`, never dispatched here.
    fn check_empty_then_if(node: &Node<'_>, ctx: &mut Context<'_>) {
        let if_node = node.as_if_node().expect("kind matched");
        if if_node.statements().is_some() {
            return;
        }
        let Some(name) = read_var_name(&if_node.predicate()) else { return };
        let Some(else_node) = if_node.subsequent().and_then(|s| s.as_else_node()) else { return };
        Self::check_body_assignment(node, name, else_node.statements(), ctx);
    }

    /// RuboCop's `on_if` reached through the same `unless_assignment?`
    /// matcher, for a literal/modifier `unless`.
    fn check_unless(node: &Node<'_>, ctx: &mut Context<'_>) {
        let unless_node = node.as_unless_node().expect("kind matched");
        if unless_node.else_clause().is_some() {
            return;
        }
        let Some(name) = read_var_name(&unless_node.predicate()) else { return };
        Self::check_body_assignment(node, name, unless_node.statements(), ctx);
    }

    /// RuboCop's `take_variable_and_default_from_unless`'s reachable path
    /// (`node.if_branch` is always absent whenever `unless_assignment?`
    /// matches, so only the `node.else_branch` case ever executes):
    /// `body` must be a single assignment to `name`, whose own name becomes
    /// `variable` and whose value becomes `default`.
    fn check_body_assignment(
        node: &Node<'_>,
        name: &[u8],
        body: Option<StatementsNode<'_>>,
        ctx: &mut Context<'_>,
    ) {
        let Some(assignment) = body.and_then(|s| single_statement(&s)) else { return };
        let Some((assign_name, value)) = assignment_name_value(&assignment) else { return };
        if assign_name != name {
            return;
        }
        let default_src = ctx.text(value.span()).to_vec();
        Self::report(ctx, node.span(), assign_name, &default_src);
    }

    /// RuboCop's `autocorrect`: `corrector.replace(node, "#{variable} ||=
    /// #{default.source}")`.
    fn report(ctx: &mut Context<'_>, span: Span, variable: &[u8], default_src: &[u8]) {
        let mut replacement = variable.to_vec();
        replacement.extend_from_slice(b" ||= ");
        replacement.extend_from_slice(default_src);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement)],
            },
        );
    }
}

/// A `StatementsNode` with exactly one statement, standing in for that
/// statement itself. Copied privately from `ruby_ast::ext`.
fn single_statement<'pr>(stmts: &StatementsNode<'pr>) -> Option<Node<'pr>> {
    let body = stmts.body();
    if body.len() == 1 {
        body.first()
    } else {
        None
    }
}

/// A variable write node's `(name, value)`, for `lvasgn`/`ivasgn`/
/// `cvasgn`/`gvasgn` only -- mirrors upstream's `{lvasgn ivasgn cvasgn
/// gvasgn}` pattern alternation.
fn assignment_name_value<'pr>(node: &Node<'pr>) -> Option<(&'pr [u8], Node<'pr>)> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => {
            node.as_local_variable_write_node().map(|w| (w.name().as_slice(), w.value()))
        }
        NodeKind::InstanceVariableWriteNode => {
            node.as_instance_variable_write_node().map(|w| (w.name().as_slice(), w.value()))
        }
        NodeKind::ClassVariableWriteNode => {
            node.as_class_variable_write_node().map(|w| (w.name().as_slice(), w.value()))
        }
        NodeKind::GlobalVariableWriteNode => {
            node.as_global_variable_write_node().map(|w| (w.name().as_slice(), w.value()))
        }
        _ => None,
    }
}

/// A variable read node's name, for `lvar`/`ivar`/`cvar`/`gvar` only --
/// mirrors upstream's `{lvar ivar cvar gvar} _var` pattern alternation.
fn read_var_name<'pr>(node: &Node<'pr>) -> Option<&'pr [u8]> {
    match node.kind() {
        NodeKind::LocalVariableReadNode => {
            node.as_local_variable_read_node().map(|n| n.name().as_slice())
        }
        NodeKind::InstanceVariableReadNode => {
            node.as_instance_variable_read_node().map(|n| n.name().as_slice())
        }
        NodeKind::ClassVariableReadNode => {
            node.as_class_variable_read_node().map(|n| n.name().as_slice())
        }
        NodeKind::GlobalVariableReadNode => {
            node.as_global_variable_read_node().map(|n| n.name().as_slice())
        }
        _ => None,
    }
}

/// RuboCop's `ternary_assignment?` applied to an assignment's own value:
/// an `IfNode` whose predicate and single then-statement both read `name`,
/// and whose `subsequent` is a plain `ElseNode` (not another `IfNode`, i.e.
/// not an `elsif` -- `return if else_branch.if_type?`), returning the span
/// of that `else`'s body (`take_variable_and_default_from_ternary`'s
/// `default`). A missing/empty `else` body mirrors a `nil` (falsy)
/// `$_` capture: no offense.
fn ternary_default_span(name: &[u8], value: &Node<'_>) -> Option<Span> {
    let if_node = value.as_if_node()?;
    if read_var_name(&if_node.predicate())? != name {
        return None;
    }
    let then_stmt = if_node.statements().and_then(|s| single_statement(&s))?;
    if read_var_name(&then_stmt)? != name {
        return None;
    }
    let else_node = if_node.subsequent().and_then(|s| s.as_else_node())?;
    Some(else_node.statements()?.as_node().span())
}
