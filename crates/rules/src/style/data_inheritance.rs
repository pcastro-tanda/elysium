//! `Style/DataInheritance`, ported from RuboCop's
//! `lib/rubocop/cop/style/data_inheritance.rb`.
//!
//! Unlike whitequark (where a call followed by a block is itself a `block`
//! node, so a superclass written `Data.define(...) do ... end` reaches
//! `data_define?` as that wrapping `block` node), Prism keeps the call as a
//! plain [`NodeKind::CallNode`] whose own span already extends through its
//! attached block; the block itself is reached via [`CallNode::block`].
//! [`is_data_define`] therefore only ever needs to check a `CallNode`, and
//! [`correct_parent`] reaches the block (if any) the same way.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, CallNode, ClassNode};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// RuboCop's `MSG`.
const MSG: &str = "Don't extend an instance initialized by `Data.define`. \
Use a block to customize the class.";

/// Checks for inheritance from Data.define.
#[derive(Debug, Clone)]
pub struct DataInheritance;

impl Rule for DataInheritance {
    const META: RuleMeta = RuleMeta {
        name: "Style/DataInheritance",
        department: Department::Style,
        summary: "Checks for inheritance from Data.define.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(class) = node.as_class_node() else { return };
        let Some(parent_class) = class.superclass() else { return };
        let Some(call) = parent_class.as_call_node() else { return };
        if !is_data_define_call(&call) {
            return;
        }
        if defines_constants(class.body()) {
            return;
        }

        let mut edits = vec![Edit::delete(ctx.with_surrounding_space(
            class.class_keyword_loc().span(),
            Side::Both,
            false,
            false,
        ))];
        if let Some(op) = class.inheritance_operator_loc() {
            edits.push(Edit::replace(op.span(), b"=".to_vec()));
        }
        correct_parent(&call, &parent_class, &class, ctx, &mut edits);

        ctx.report_with_fix(
            &Self::META,
            parent_class.span(),
            MSG,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// `(send (const {nil? cbase} :Data) :define ...)`.
fn is_data_define_call(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() != b"define" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    is_bare_or_toplevel_const(&receiver, b"Data")
}

/// `(const {nil? (cbase)} :Name)`.
fn is_bare_or_toplevel_const(node: &Node<'_>, name: &[u8]) -> bool {
    if let Some(read) = node.as_constant_read_node() {
        return read.name().as_slice() == name;
    }
    if let Some(path) = node.as_constant_path_node() {
        return path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == name);
    }
    false
}

/// `defines_constants?`: any descendant of `body` is a constant
/// assignment, nested class, or nested module.
fn defines_constants(body: Option<Node<'_>>) -> bool {
    let Some(body) = body else { return false };
    let mut found = false;
    each_descendant(&body, &mut |n| {
        if matches!(
            n.kind(),
            NodeKind::ConstantWriteNode
                | NodeKind::ConstantPathWriteNode
                | NodeKind::ClassNode
                | NodeKind::ModuleNode
        ) {
            found = true;
        }
    });
    found
}

/// `correct_parent`.
fn correct_parent(
    call: &CallNode<'_>,
    parent_class: &Node<'_>,
    class_node: &ClassNode<'_>,
    ctx: &Context<'_>,
    edits: &mut Vec<Edit>,
) {
    if let Some(block) = call.block().and_then(|b| b.as_block_node()) {
        if is_braces(&block) {
            edits.push(Edit::replace(block.opening_loc().span(), b"do".to_vec()));
        }
        edits.push(Edit::delete(ctx.with_surrounding_space(
            block.closing_loc().span(),
            Side::Both,
            false,
            false,
        )));
    } else if class_node.body().is_none() {
        edits.push(Edit::delete(range_for_empty_class_body(class_node, parent_class, ctx)));
    } else {
        edits.push(Edit::insert(parent_class.span().end, b" do".to_vec()));
    }
}

/// `parent.braces?`: a `BlockNode` opened with `{` rather than `do`.
fn is_braces(block: &BlockNode<'_>) -> bool {
    block.opening_loc().as_slice().starts_with(b"{")
}

/// `range_for_empty_class_body`.
fn range_for_empty_class_body(
    class_node: &ClassNode<'_>,
    data_define: &Node<'_>,
    ctx: &Context<'_>,
) -> Span {
    let class_span = class_node.as_node().span();
    if ctx.is_single_line(class_span) {
        Span::new(data_define.span().end, class_span.end)
    } else {
        ctx.whole_lines(class_node.end_keyword_loc().span())
    }
}
