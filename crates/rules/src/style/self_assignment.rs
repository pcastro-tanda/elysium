//! `Style/SelfAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/style/self_assignment.rb`.
//!
//! # Node shapes
//!
//! `x = x + 1` is a `LocalVariableWriteNode` whose `value()` is a `CallNode`
//! (`receiver: LocalVariableReadNode(x)`, `name: :+`, one argument) --
//! binary-operator syntax and explicit dot-call syntax (`x.+(1)`) produce the
//! identical `CallNode` shape in Prism, so both are handled by the same
//! `CallNode` branch below. `x = x || y` / `x = x && y` are `OrNode`/`AndNode`
//! (`operator_keyword?` upstream); `x = x and y` / `x = x or y` bind looser
//! than `=`, so the write node's `value()` there is just `x` and neither
//! branch matches -- no offense, matching upstream (the keyword forms are
//! never in `OPS` nor tested).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Binary operator methods eligible for self-assignment shorthand.
const OPS: &[&[u8]] = &[b"+", b"-", b"*", b"**", b"/", b"%", b"^", b"<<", b">>", b"|", b"&"];

/// Checks for places where self-assignment shorthand should have been used.
#[derive(Debug, Clone)]
pub struct SelfAssignment;

impl Rule for SelfAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Style/SelfAssignment",
        department: Department::Style,
        summary: "Checks for places where self-assignment shorthand should have been used.",
        explanation: "\
Enforces the use of the shorthand for self-assignment.

```ruby
# bad
x = x + 1

# good
x += 1
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::LocalVariableWriteNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::ClassVariableWriteNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some((name, operator_loc, value)) = write_parts(node) else { return };

        let receiver_matches = |candidate: &Node<'_>| -> bool {
            match (node.kind(), candidate.kind()) {
                (NodeKind::LocalVariableWriteNode, NodeKind::LocalVariableReadNode) => {
                    candidate.as_local_variable_read_node().expect("kind matched").name().as_slice()
                        == name
                }
                (NodeKind::InstanceVariableWriteNode, NodeKind::InstanceVariableReadNode) => {
                    candidate
                        .as_instance_variable_read_node()
                        .expect("kind matched")
                        .name()
                        .as_slice()
                        == name
                }
                (NodeKind::ClassVariableWriteNode, NodeKind::ClassVariableReadNode) => {
                    candidate.as_class_variable_read_node().expect("kind matched").name().as_slice()
                        == name
                }
                _ => false,
            }
        };

        let value_span = value.span();
        let (method, new_rhs_span): (Vec<u8>, Span) = if let Some(call) = value.as_call_node() {
            let Some(args) = call.arguments() else { return };
            let list = args.arguments();
            if list.len() != 1 {
                return;
            }
            let method = call.name().as_slice();
            if !OPS.contains(&method) {
                return;
            }
            let Some(receiver) = call.receiver() else { return };
            if !receiver_matches(&receiver) {
                return;
            }
            (method.to_vec(), list.first().expect("len checked above").span())
        } else {
            let (left, right, op_loc) = if let Some(or_node) = value.as_or_node() {
                (or_node.left(), or_node.right(), or_node.operator_loc())
            } else if let Some(and_node) = value.as_and_node() {
                (and_node.left(), and_node.right(), and_node.operator_loc())
            } else {
                return;
            };
            if !receiver_matches(&left) {
                return;
            }
            (ctx.text(op_loc.span()).to_vec(), right.span())
        };

        let method_str = String::from_utf8_lossy(&method);
        let msg = format!("Use self-assignment shorthand `{method_str}=`.");
        let new_rhs_text = ctx.text(new_rhs_span).to_vec();
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            msg,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::insert(operator_loc.span().start, method),
                    Edit::replace(value_span, new_rhs_text),
                ],
            },
        );
    }
}

/// Extracts `(name, operator_loc, value)` from a `LocalVariableWriteNode` /
/// `InstanceVariableWriteNode` / `ClassVariableWriteNode`; upstream's
/// `on_lvasgn` / `on_ivasgn` / `on_cvasgn`.
fn write_parts<'pr>(node: &Node<'pr>) -> Option<(&'pr [u8], ruby_ast::Location<'pr>, Node<'pr>)> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => {
            let w = node.as_local_variable_write_node().expect("kind matched");
            Some((w.name().as_slice(), w.operator_loc(), w.value()))
        }
        NodeKind::InstanceVariableWriteNode => {
            let w = node.as_instance_variable_write_node().expect("kind matched");
            Some((w.name().as_slice(), w.operator_loc(), w.value()))
        }
        NodeKind::ClassVariableWriteNode => {
            let w = node.as_class_variable_write_node().expect("kind matched");
            Some((w.name().as_slice(), w.operator_loc(), w.value()))
        }
        _ => None,
    }
}
