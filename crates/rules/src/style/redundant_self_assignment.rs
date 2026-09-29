//! `Style/RedundantSelfAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_self_assignment.rb`.
//!
//! Two shapes are checked, matching upstream's `on_lvasgn` (aliased to
//! `on_ivasgn`/`on_cvasgn`/`on_gvasgn`) and `on_send` (aliased to
//! `on_csend`):
//!
//! - A plain variable write (`foo = foo.concat(ary)`): Prism's
//!   [`NodeKind::LocalVariableWriteNode`]/[`NodeKind::InstanceVariableWriteNode`]/
//!   [`NodeKind::ClassVariableWriteNode`]/[`NodeKind::GlobalVariableWriteNode`],
//!   handled in [`check_variable_write`].
//! - An attribute writer call (`other.foo = other.foo.concat(ary)`): a
//!   [`NodeKind::CallNode`] whose `equal_loc` is set, the same
//!   `setter_method?`/`assignment_method?` approximation used elsewhere in
//!   this crate (e.g. `Style/ParenthesesAsGroupedExpression`), handled in
//!   [`check_attribute_write`]. whitequark's `send`/`csend` split collapses
//!   into one `CallNode` kind here, so both `other.foo = ...` and
//!   `other&.foo = ...` (and a `&.`-chained rhs) are covered by the same
//!   code, matching upstream's `alias on_csend on_send`.
//!
//! In both shapes, whitequark's `any_block`/`call` rhs type check is just
//! "the value is a `CallNode`" here: a call with a literal block (e.g.
//! `foo = foo.delete_if { true }`) is never a separate wrapping node in
//! Prism, unlike whitequark's `block` node.
//!
//! `node.lhs`/`receiver.children.first` (rubocop-ast's `AsgnNode#lhs`, the
//! *name* of the assigned variable, not a node) becomes a `ConstantId`
//! byte-slice comparison against the read node's own name in
//! [`same_variable`]. The attribute-write shape's receiver identity check
//! (`%1` reused across the node pattern) is approximated the same way as
//! `Style/RedundantCondition`'s `receiver_eq`: source-text equality, copied
//! privately here per the porting kit's file-isolation rule.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `METHODS_RETURNING_SELF`.
const METHODS_RETURNING_SELF: &[&[u8]] = &[
    b"append",
    b"clear",
    b"collect!",
    b"compare_by_identity",
    b"concat",
    b"delete_if",
    b"fill",
    b"initialize_copy",
    b"insert",
    b"keep_if",
    b"map!",
    b"merge!",
    b"prepend",
    b"push",
    b"rehash",
    b"replace",
    b"reverse!",
    b"rotate!",
    b"shuffle!",
    b"sort!",
    b"sort_by!",
    b"transform_keys!",
    b"transform_values!",
    b"unshift",
    b"update",
];

fn method_returning_self(name: &[u8]) -> bool {
    METHODS_RETURNING_SELF.contains(&name)
}

fn message(method_name: &[u8]) -> String {
    format!(
        "Redundant self assignment detected. Method `{}` modifies its receiver in place.",
        String::from_utf8_lossy(method_name)
    )
}

/// Checks for places where redundant assignments are made for in place modification methods.
#[derive(Debug, Clone)]
pub struct RedundantSelfAssignment;

impl Rule for RedundantSelfAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantSelfAssignment",
        department: Department::Style,
        summary: "Checks for places where redundant assignments are made for in place modification methods.",
        explanation: "\
Checks for places where redundant assignments are made for in place
modification methods.

```ruby
# bad
args = args.concat(ary)
hash = hash.merge!(other)

# good
args.concat(foo)
args += foo
hash.merge!(other)

# good
foo.concat(ary)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::LocalVariableWriteNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::ClassVariableWriteNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::CallNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                check_attribute_write(node, &call, ctx);
            }
            _ => check_variable_write(node, ctx),
        }
    }
}

/// One of the four plain-variable assignment node kinds, normalized to its
/// name and rhs value (rubocop-ast's `AsgnNode#lhs`/`#rhs`).
struct VariableWrite<'pr> {
    name: &'pr [u8],
    value: Node<'pr>,
    operator: Span,
}

fn variable_write<'pr>(node: &Node<'pr>) -> Option<VariableWrite<'pr>> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => {
            let w = node.as_local_variable_write_node()?;
            Some(VariableWrite {
                name: w.name().as_slice(),
                value: w.value(),
                operator: w.operator_loc().span(),
            })
        }
        NodeKind::InstanceVariableWriteNode => {
            let w = node.as_instance_variable_write_node()?;
            Some(VariableWrite {
                name: w.name().as_slice(),
                value: w.value(),
                operator: w.operator_loc().span(),
            })
        }
        NodeKind::ClassVariableWriteNode => {
            let w = node.as_class_variable_write_node()?;
            Some(VariableWrite {
                name: w.name().as_slice(),
                value: w.value(),
                operator: w.operator_loc().span(),
            })
        }
        NodeKind::GlobalVariableWriteNode => {
            let w = node.as_global_variable_write_node()?;
            Some(VariableWrite {
                name: w.name().as_slice(),
                value: w.value(),
                operator: w.operator_loc().span(),
            })
        }
        _ => None,
    }
}

/// Whether `receiver` is a read of the same kind of variable as `write_kind`,
/// with the same name (rubocop-ast's `receiver.type == receiver_type &&
/// receiver.children.first == node.lhs`).
fn same_variable(write_kind: NodeKind, name: &[u8], receiver: &Node<'_>) -> bool {
    match write_kind {
        NodeKind::LocalVariableWriteNode => {
            receiver.as_local_variable_read_node().is_some_and(|r| r.name().as_slice() == name)
        }
        NodeKind::InstanceVariableWriteNode => {
            receiver.as_instance_variable_read_node().is_some_and(|r| r.name().as_slice() == name)
        }
        NodeKind::ClassVariableWriteNode => {
            receiver.as_class_variable_read_node().is_some_and(|r| r.name().as_slice() == name)
        }
        NodeKind::GlobalVariableWriteNode => {
            receiver.as_global_variable_read_node().is_some_and(|r| r.name().as_slice() == name)
        }
        _ => false,
    }
}

/// RuboCop's `on_lvasgn` (aliased to `on_ivasgn`/`on_cvasgn`/`on_gvasgn`).
fn check_variable_write(node: &Node<'_>, ctx: &mut Context<'_>) {
    let Some(write) = variable_write(node) else { return };
    let Some(rhs) = write.value.as_call_node() else { return };
    if !method_returning_self(rhs.name().as_slice()) {
        return;
    }
    let Some(receiver) = rhs.receiver() else { return };
    if !same_variable(node.kind(), write.name, &receiver) {
        return;
    }

    let msg = message(rhs.name().as_slice());
    let replacement = ctx.text(write.value.span()).to_vec();
    ctx.report_with_fix(
        &RedundantSelfAssignment::META,
        write.operator,
        msg,
        Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(node.span(), replacement)],
        },
    );
}

/// RuboCop's `on_send` (aliased to `on_csend`).
fn check_attribute_write(node: &Node<'_>, call: &CallNode<'_>, ctx: &mut Context<'_>) {
    let Some(eq_loc) = call.equal_loc() else { return };
    let name = call.name();
    let name = name.as_slice();
    let Some(receiver_name) = name.strip_suffix(b"=") else { return };

    let Some(first_arg) = call.arguments().and_then(|a| a.arguments().first()) else { return };
    let Some(rhs) = first_arg.as_call_node() else { return };
    if !method_returning_self(rhs.name().as_slice()) {
        return;
    }
    let Some(rhs_receiver) = rhs.receiver() else { return };
    let Some(rhs_receiver_call) = rhs_receiver.as_call_node() else { return };
    if rhs_receiver_call.name().as_slice() != receiver_name {
        return;
    }
    if !receiver_eq(ctx, call.receiver(), rhs_receiver_call.receiver()) {
        return;
    }

    let msg = message(rhs.name().as_slice());
    // RuboCop's `correction_range`: from the start of the whole node through
    // the start of its first argument, i.e. the `receiver.attr = ` prefix.
    let delete = Span::new(node.span().start, first_arg.span().start);
    ctx.report_with_fix(
        &RedundantSelfAssignment::META,
        eq_loc.span(),
        msg,
        Fix { applicability: Applicability::Unsafe, edits: vec![Edit::delete(delete)] },
    );
}

/// `Style/RedundantCondition`'s `receiver_eq`, copied privately per the
/// porting kit's file-isolation rule: an approximation of node identity via
/// source-text equality.
fn receiver_eq(ctx: &Context<'_>, a: Option<Node<'_>>, b: Option<Node<'_>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => ctx.text(x.span()) == ctx.text(y.span()),
        _ => false,
    }
}
