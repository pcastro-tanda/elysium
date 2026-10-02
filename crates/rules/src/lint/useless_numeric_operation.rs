//! `Lint/UselessNumericOperation`, ported from RuboCop's
//! `lib/rubocop/cop/lint/useless_numeric_operation.rb`.
//!
//! # A qualified-constant blind spot
//!
//! Upstream's `useless_operation?`/`useless_abbreviated_assignment?`
//! node-pattern matchers (`{lvar ivar cvar gvar const (send nil? _)}`/
//! `{lvasgn ivasgn cvasgn gvasgn casgn}`) match a bare `const`/`casgn`
//! receiver or target -- whitequark's `:const`/`:casgn` node types cover
//! both a bare `Foo` and a qualified `Foo::Bar` uniformly (the latter
//! recursing through a `namespace` child of the same type). Prism splits
//! these into `ConstantReadNode`/`ConstantOperatorWriteNode` (bare) versus
//! `ConstantPathNode`/`ConstantPathOperatorWriteNode` (qualified), so this
//! rule -- subscribing only to the bare kinds, per the `whitequark ->
//! Prism` trap doc -- does not fire on a qualified constant receiver or
//! op-assign target (e.g. `Foo::BAR + 0`, `Foo::BAR *= 1`). See
//! `blind_spots`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, IntegerNode};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Do not apply inconsequential numeric operations to variables.";

/// Checks for useless numeric operations.
#[derive(Debug, Clone)]
pub struct UselessNumericOperation;

impl Rule for UselessNumericOperation {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UselessNumericOperation",
        department: Department::Lint,
        summary: "Checks for useless numeric operations.",
        explanation: "\
Certain numeric operations have no impact, being: Adding or subtracting 0,
multiplying or dividing by 1 or raising to the power of 1. These are
probably leftover from debugging, or are mistakes.

```ruby
# bad
x + 0
x - 0
x * 1
x / 1
x ** 1

# good
x

# bad
x += 0
x -= 0
x *= 1
x /= 1
x **= 1

# good
x = x
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::LocalVariableOperatorWriteNode,
            NodeKind::InstanceVariableOperatorWriteNode,
            NodeKind::ClassVariableOperatorWriteNode,
            NodeKind::GlobalVariableOperatorWriteNode,
            NodeKind::ConstantOperatorWriteNode,
        ],
        config: &[],
        blind_spots: "\
A qualified constant receiver/target (`Foo::BAR + 0`, `Foo::BAR *= 1`) is \
not matched: Prism splits whitequark's single `const`/`casgn` node type \
(which recurses to cover both a bare and a qualified constant) into \
`ConstantReadNode`/`ConstantOperatorWriteNode` (bare) versus \
`ConstantPathNode`/`ConstantPathOperatorWriteNode` (qualified); this rule, \
matching only the bare kinds per the node-pattern's `const`/`casgn` \
alternative, never sees the qualified ones.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                check_call(node.span(), &call, ctx);
            }
            Node::LocalVariableOperatorWriteNode { .. } => {
                let w = node.as_local_variable_operator_write_node().expect("kind matched");
                check_op_asgn(
                    node.span(),
                    w.name().as_slice(),
                    w.binary_operator().as_slice(),
                    &w.value(),
                    ctx,
                );
            }
            Node::InstanceVariableOperatorWriteNode { .. } => {
                let w = node.as_instance_variable_operator_write_node().expect("kind matched");
                check_op_asgn(
                    node.span(),
                    w.name().as_slice(),
                    w.binary_operator().as_slice(),
                    &w.value(),
                    ctx,
                );
            }
            Node::ClassVariableOperatorWriteNode { .. } => {
                let w = node.as_class_variable_operator_write_node().expect("kind matched");
                check_op_asgn(
                    node.span(),
                    w.name().as_slice(),
                    w.binary_operator().as_slice(),
                    &w.value(),
                    ctx,
                );
            }
            Node::GlobalVariableOperatorWriteNode { .. } => {
                let w = node.as_global_variable_operator_write_node().expect("kind matched");
                check_op_asgn(
                    node.span(),
                    w.name().as_slice(),
                    w.binary_operator().as_slice(),
                    &w.value(),
                    ctx,
                );
            }
            Node::ConstantOperatorWriteNode { .. } => {
                let w = node.as_constant_operator_write_node().expect("kind matched");
                check_op_asgn(
                    node.span(),
                    w.name().as_slice(),
                    w.binary_operator().as_slice(),
                    &w.value(),
                    ctx,
                );
            }
            _ => {}
        }
    }
}

/// RuboCop's `useless_operation?` matcher plus `useless?`, for
/// `on_send`/`on_csend` (a `CallNode` covers both; safe navigation is not
/// excluded, matching upstream's `alias on_csend on_send`).
fn check_call(call_span: Span, call: &CallNode<'_>, ctx: &mut Context<'_>) {
    let Some(receiver) = call.receiver() else { return };
    if !is_useless_receiver(&receiver) {
        return;
    }
    let Some(arguments) = call.arguments() else { return };
    let args = arguments.arguments();
    if args.len() != 1 {
        return;
    }
    let Some(int_node) = args.first().and_then(|a| a.as_integer_node()) else { return };
    if !is_useless_operation(call.name().as_slice(), &int_node) {
        return;
    }
    let replacement = ctx.text(receiver.span()).to_vec();
    ctx.report_with_fix(
        &UselessNumericOperation::META,
        call_span,
        MSG,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(call_span, replacement.into_boxed_slice())],
        },
    );
}

/// RuboCop's `useless_operation?` receiver alternative: `{lvar ivar cvar
/// gvar const (send nil? _)}`.
fn is_useless_receiver(node: &Node<'_>) -> bool {
    match node {
        Node::LocalVariableReadNode { .. }
        | Node::InstanceVariableReadNode { .. }
        | Node::ClassVariableReadNode { .. }
        | Node::GlobalVariableReadNode { .. }
        | Node::ConstantReadNode { .. } => true,
        Node::CallNode { .. } => {
            let call = node.as_call_node().expect("kind matched");
            call.receiver().is_none() && call.arguments().is_none() && call.block().is_none()
        }
        _ => false,
    }
}

/// RuboCop's `useless_abbreviated_assignment?` body, shared by every
/// `*OperatorWriteNode` kind: replaces the whole node with `"name = name"`
/// when useless.
fn check_op_asgn(
    node_span: Span,
    name: &[u8],
    operator: &[u8],
    value: &Node<'_>,
    ctx: &mut Context<'_>,
) {
    let Some(int_node) = value.as_integer_node() else { return };
    if !is_useless_operation(operator, &int_node) {
        return;
    }
    let mut replacement = name.to_vec();
    replacement.extend_from_slice(b" = ");
    replacement.extend_from_slice(name);
    ctx.report_with_fix(
        &UselessNumericOperation::META,
        node_span,
        MSG,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node_span, replacement.into_boxed_slice())],
        },
    );
}

/// RuboCop's `useless?`: adding/subtracting 0, or multiplying/dividing/
/// exponentiating by 1.
fn is_useless_operation(operator: &[u8], int: &IntegerNode<'_>) -> bool {
    if is_integer_value(int, 0) {
        matches!(operator, b"+" | b"-")
    } else if is_integer_value(int, 1) {
        matches!(operator, b"*" | b"/" | b"**")
    } else {
        false
    }
}

/// Whether an `IntegerNode`'s arbitrary-precision value equals `target`
/// (only ever called with `0` or `1`, both of which fit in a single `u32`
/// digit).
fn is_integer_value(int: &IntegerNode<'_>, target: u32) -> bool {
    let value = int.value();
    let (negative, digits) = value.to_u32_digits();
    if negative {
        return false;
    }
    if target == 0 {
        digits.iter().all(|&d| d == 0)
    } else {
        digits.len() == 1 && digits[0] == target
    }
}
