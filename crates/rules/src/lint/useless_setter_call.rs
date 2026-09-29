//! `Lint/UselessSetterCall`, ported from RuboCop's
//! `lib/rubocop/cop/lint/useless_setter_call.rb`.
//!
//! Upstream tracks assignments across a method body with a custom
//! `MethodVariableTracker`, built on whitequark's unified `masgn`/`or_asgn`/
//! `and_asgn`/`op_asgn` node types, each of which wraps a generic lhs
//! assignment node (`lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`) regardless of
//! whether the actual target is a plain variable or an attribute/index
//! writer. Prism instead fuses the lhs kind straight into the node kind
//! (`LocalVariableOrWriteNode` vs. `CallOrWriteNode`, ...), so [`scan`] only
//! matches the four variable-write families
//! (`{Local,Instance,Class,Global}Variable{Write,OrWrite,AndWrite,
//! OperatorWrite}Node`). Attribute/index compound writes (`CallOrWriteNode`,
//! `IndexOperatorWriteNode`, ...) fall through to the "not an assignment"
//! arm, which reproduces upstream's own
//! `return unless ASSIGNMENT_TYPES.include?(lhs_node.type)` guard -- no
//! `throw :skip_children`, so the scan continues into that node's children
//! -- without an extra check.
//!
//! `setter_method?` (RuboCop-AST's `MethodDispatchNode#setter_method?`,
//! `loc?(:operator)`) is approximated the same way as elsewhere in this
//! crate: Prism's `CallNode::equal_loc` presence (see
//! `Style::ParenthesesAsGroupedExpression`).
//!
//! The trailing expression of a `def`/`defs` body: whitequark elides a
//! single-statement `begin`, so `last_expression` is either the lone body
//! node or the last child of an (implicit) `begin`. Prism always wraps a
//! multi-statement body in a `StatementsNode`, so this unwraps to that
//! node's last child, or the body itself when it is not a `StatementsNode`
//! -- including a single explicit `begin...rescue...end` (upstream's
//! `kwbegin`, never unwrapped either, since its type isn't `:begin`).

use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};

/// Checks for useless setter call to a local variable.
#[derive(Debug, Clone)]
pub struct UselessSetterCall;

impl Rule for UselessSetterCall {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UselessSetterCall",
        department: Department::Lint,
        summary: "Checks for useless setter call to a local variable.",
        explanation: "\
Checks for setter call to local variable as the final expression of a \
function definition.

There are edge cases in which the local variable references a value that \
is also accessible outside the local scope. This is not detected by the \
cop, and it can yield a false positive. As well, autocorrection is unsafe \
because the method's return value will be changed.",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        let Some(body) = def.body() else { return };

        let Some(last_expr) = last_expression(&body) else { return };
        let Some(call) = last_expr.as_call_node() else { return };
        if call.equal_loc().is_none() {
            return; // setter_method?
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(lvar) = receiver.as_local_variable_read_node() else { return };

        let mut local: HashMap<Vec<u8>, bool> = HashMap::new();
        scan(&body, &mut local);
        if !local.get(lvar.name().as_slice()).copied().unwrap_or(false) {
            return;
        }

        let span = receiver.location().span();
        let variable_text = String::from_utf8_lossy(receiver.location().as_slice()).into_owned();
        let msg = format!("Useless setter call to local variable `{variable_text}`.");

        let indent = leading_whitespace(ctx, last_expr.span().start);
        let insert_text = format!("\n{indent}{variable_text}").into_bytes();

        ctx.report_with_fix(
            &Self::META,
            span,
            msg,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::insert(last_expr.span().end, insert_text)],
            },
        );
    }
}

/// Upstream's `last_expression`: the last statement of a (possibly
/// single-statement) method body. See the module doc for the whitequark
/// `begin`/Prism `StatementsNode` mapping.
fn last_expression<'pr>(body: &Node<'pr>) -> Option<Node<'pr>> {
    match body.as_statements_node() {
        Some(stmts) => stmts.body().last(),
        None => Some(*body),
    }
}

/// Upstream's `MethodVariableTracker#scan`: a `catch(:skip_children)`-guarded
/// recursive walk that feeds every node to `process_assignment_node`, only
/// descending into a node's children when that call didn't request a skip.
fn scan(node: &Node<'_>, local: &mut HashMap<Vec<u8>, bool>) {
    if process_assignment_node(node, local) {
        return;
    }
    for_each_child(node, |child| scan(child, local));
}

/// Upstream's `MethodVariableTracker#process_assignment_node`, `true` when
/// the equivalent whitequark branch would `throw :skip_children`.
fn process_assignment_node(node: &Node<'_>, local: &mut HashMap<Vec<u8>, bool>) -> bool {
    match node.kind() {
        NodeKind::MultiWriteNode => {
            process_multiple_assignment(&node.as_multi_write_node().expect("kind matched"), local);
            true
        }
        NodeKind::LocalVariableOrWriteNode => {
            let n = node.as_local_variable_or_write_node().expect("kind matched");
            process_assignment(local, n.name().as_slice().to_vec(), &n.value());
            true
        }
        NodeKind::LocalVariableAndWriteNode => {
            let n = node.as_local_variable_and_write_node().expect("kind matched");
            process_assignment(local, n.name().as_slice().to_vec(), &n.value());
            true
        }
        NodeKind::InstanceVariableOrWriteNode => {
            let n = node.as_instance_variable_or_write_node().expect("kind matched");
            process_assignment(local, n.name().as_slice().to_vec(), &n.value());
            true
        }
        NodeKind::InstanceVariableAndWriteNode => {
            let n = node.as_instance_variable_and_write_node().expect("kind matched");
            process_assignment(local, n.name().as_slice().to_vec(), &n.value());
            true
        }
        NodeKind::ClassVariableOrWriteNode => {
            let n = node.as_class_variable_or_write_node().expect("kind matched");
            process_assignment(local, n.name().as_slice().to_vec(), &n.value());
            true
        }
        NodeKind::ClassVariableAndWriteNode => {
            let n = node.as_class_variable_and_write_node().expect("kind matched");
            process_assignment(local, n.name().as_slice().to_vec(), &n.value());
            true
        }
        NodeKind::GlobalVariableOrWriteNode => {
            let n = node.as_global_variable_or_write_node().expect("kind matched");
            process_assignment(local, n.name().as_slice().to_vec(), &n.value());
            true
        }
        NodeKind::GlobalVariableAndWriteNode => {
            let n = node.as_global_variable_and_write_node().expect("kind matched");
            process_assignment(local, n.name().as_slice().to_vec(), &n.value());
            true
        }
        NodeKind::LocalVariableOperatorWriteNode => {
            let n = node.as_local_variable_operator_write_node().expect("kind matched");
            local.insert(n.name().as_slice().to_vec(), true);
            true
        }
        NodeKind::InstanceVariableOperatorWriteNode => {
            let n = node.as_instance_variable_operator_write_node().expect("kind matched");
            local.insert(n.name().as_slice().to_vec(), true);
            true
        }
        NodeKind::ClassVariableOperatorWriteNode => {
            let n = node.as_class_variable_operator_write_node().expect("kind matched");
            local.insert(n.name().as_slice().to_vec(), true);
            true
        }
        NodeKind::GlobalVariableOperatorWriteNode => {
            let n = node.as_global_variable_operator_write_node().expect("kind matched");
            local.insert(n.name().as_slice().to_vec(), true);
            true
        }
        NodeKind::LocalVariableWriteNode => {
            let n = node.as_local_variable_write_node().expect("kind matched");
            process_assignment(local, n.name().as_slice().to_vec(), &n.value());
            false
        }
        NodeKind::InstanceVariableWriteNode => {
            let n = node.as_instance_variable_write_node().expect("kind matched");
            process_assignment(local, n.name().as_slice().to_vec(), &n.value());
            false
        }
        NodeKind::ClassVariableWriteNode => {
            let n = node.as_class_variable_write_node().expect("kind matched");
            process_assignment(local, n.name().as_slice().to_vec(), &n.value());
            false
        }
        NodeKind::GlobalVariableWriteNode => {
            let n = node.as_global_variable_write_node().expect("kind matched");
            process_assignment(local, n.name().as_slice().to_vec(), &n.value());
            false
        }
        _ => false,
    }
}

/// Upstream's `MethodVariableTracker#process_multiple_assignment`: each
/// top-level destructuring slot maps to the right-hand side element at the
/// same position (rather than a flattened traversal, which would misalign a
/// nested destructuring slot such as `(a, b)` in `(a, b), c = x, y`).
fn process_multiple_assignment(
    masgn: &ruby_ast::node::MultiWriteNode<'_>,
    local: &mut HashMap<Vec<u8>, bool>,
) {
    let mut slots: Vec<Node<'_>> = masgn.lefts().iter().collect();
    if let Some(rest) = masgn.rest() {
        slots.push(rest);
    }
    slots.extend(masgn.rights().iter());

    let value = masgn.value();
    let rhs_elements = value.as_array_node().map(|a| a.elements());

    for (index, lhs) in slots.into_iter().enumerate() {
        let Some(name) = variable_target_name(&lhs) else { continue };

        if let Some(rhs_node) = rhs_elements.as_ref().and_then(|els| els.iter().nth(index)) {
            process_assignment(local, name, &rhs_node);
        } else {
            local.insert(name, true);
        }
    }
}

/// Upstream's `ASSIGNMENT_TYPES.include?(lhs_node.type)` check for a masgn
/// slot: the slot's variable name when it's a plain
/// local/instance/class/global variable target (nested destructuring,
/// splats, and attribute/index targets are not tracked, matching upstream).
fn variable_target_name(node: &Node<'_>) -> Option<Vec<u8>> {
    match node.kind() {
        NodeKind::LocalVariableTargetNode => {
            Some(node.as_local_variable_target_node()?.name().as_slice().to_vec())
        }
        NodeKind::InstanceVariableTargetNode => {
            Some(node.as_instance_variable_target_node()?.name().as_slice().to_vec())
        }
        NodeKind::ClassVariableTargetNode => {
            Some(node.as_class_variable_target_node()?.name().as_slice().to_vec())
        }
        NodeKind::GlobalVariableTargetNode => {
            Some(node.as_global_variable_target_node()?.name().as_slice().to_vec())
        }
        _ => None,
    }
}

/// Upstream's `MethodVariableTracker#process_assignment`.
fn process_assignment(local: &mut HashMap<Vec<u8>, bool>, name: Vec<u8>, rhs: &Node<'_>) {
    let value = if let Some(rhs_name) = variable_read_name(rhs) {
        local.get(&rhs_name).copied().unwrap_or(false)
    } else {
        is_constructor(rhs)
    };
    local.insert(name, value);
}

/// Upstream's `node.variable?` (`VARIABLES = %i[ivar gvar cvar lvar]`): the
/// name of a plain local/instance/class/global variable *read*.
fn variable_read_name(node: &Node<'_>) -> Option<Vec<u8>> {
    match node.kind() {
        NodeKind::LocalVariableReadNode => {
            Some(node.as_local_variable_read_node()?.name().as_slice().to_vec())
        }
        NodeKind::InstanceVariableReadNode => {
            Some(node.as_instance_variable_read_node()?.name().as_slice().to_vec())
        }
        NodeKind::ClassVariableReadNode => {
            Some(node.as_class_variable_read_node()?.name().as_slice().to_vec())
        }
        NodeKind::GlobalVariableReadNode => {
            Some(node.as_global_variable_read_node()?.name().as_slice().to_vec())
        }
        _ => None,
    }
}

/// Upstream's `MethodVariableTracker#constructor?`: a basic literal, or a
/// plain (non-safe-navigation) `.new` call.
fn is_constructor(node: &Node<'_>) -> bool {
    if is_literal_kind(node.kind()) {
        return true;
    }
    let Some(call) = node.as_call_node() else { return false };
    if call.is_safe_navigation() {
        return false; // `send_type?` (whitequark distinguishes `send`/`csend`)
    }
    call.name().as_slice() == b"new"
}

/// RuboCop-AST's `LITERALS`, the node kinds `Node#literal?` recognizes.
fn is_literal_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::RangeNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
    )
}

/// Upstream's `Base#indent`: the whitespace at the start of the line
/// containing `offset`.
fn leading_whitespace(ctx: &Context<'_>, offset: u32) -> String {
    let line = ctx.line_col(offset).line;
    let text = ctx.line_text(line);
    let end = text.iter().take_while(|&&b| b == b' ' || b == b'\t').count();
    String::from_utf8_lossy(&text[..end]).into_owned()
}
