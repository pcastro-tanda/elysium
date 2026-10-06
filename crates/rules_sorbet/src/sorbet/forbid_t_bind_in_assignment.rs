//! `Sorbet/ForbidTBindInAssignment`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_t_bind_in_assignment.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str =
    "Do not assign the result of `T.bind`; it also changes the type of its first argument.";

/// Forbid assigning the result of T.bind.
#[derive(Debug, Clone)]
pub struct ForbidTBindInAssignment;

impl Rule for ForbidTBindInAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidTBindInAssignment",
        department: Department::Sorbet,
        summary: "Forbid assigning the result of T.bind.",
        explanation: "Disallows assigning the result of `T.bind`.\n\n`T.bind` changes the type of its first argument and returns that argument. Assigning its result can therefore unintentionally change the inferred type of both the assignment target and the first argument.\n\nAuto-correction is unsafe because replacing `T.bind` with `T.cast` removes the scope-wide type rebind of the first argument.\n\n```ruby\n# bad\nfoo = T.bind(self, Integer)\n\n# good\nfoo = T.cast(self, Integer)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::LocalVariableWriteNode, NodeKind::InstanceVariableWriteNode, NodeKind::ClassVariableWriteNode, NodeKind::GlobalVariableWriteNode, NodeKind::ConstantWriteNode, NodeKind::ConstantPathWriteNode, NodeKind::LocalVariableOrWriteNode, NodeKind::LocalVariableAndWriteNode, NodeKind::LocalVariableOperatorWriteNode, NodeKind::InstanceVariableOrWriteNode, NodeKind::InstanceVariableAndWriteNode, NodeKind::InstanceVariableOperatorWriteNode, NodeKind::ClassVariableOrWriteNode, NodeKind::ClassVariableAndWriteNode, NodeKind::ClassVariableOperatorWriteNode, NodeKind::GlobalVariableOrWriteNode, NodeKind::GlobalVariableAndWriteNode, NodeKind::GlobalVariableOperatorWriteNode, NodeKind::ConstantOrWriteNode, NodeKind::ConstantAndWriteNode, NodeKind::ConstantOperatorWriteNode, NodeKind::ConstantPathOrWriteNode, NodeKind::ConstantPathAndWriteNode, NodeKind::ConstantPathOperatorWriteNode, NodeKind::CallOrWriteNode, NodeKind::IndexOrWriteNode, NodeKind::CallAndWriteNode, NodeKind::IndexAndWriteNode, NodeKind::CallOperatorWriteNode, NodeKind::IndexOperatorWriteNode, NodeKind::MultiWriteNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(value) = assigned_value(node) else { return };
        // `(send (const nil? :T) :bind _ _)`.
        let Some(call) = value.as_call_node() else { return };
        if call.name().as_slice() != b"bind"
            || call.is_safe_navigation()
            || call.block().is_some_and(|block| block.as_block_node().is_some())
            || !call.receiver().is_some_and(|receiver| is_const_t(&receiver))
        {
            return;
        }
        // A `&blk` argument is a `block_pass` child of the `send` in whitequark.
        let block_pass = usize::from(call.block().is_some());
        let argument_count = call.arguments().map_or(0, |args| args.arguments().iter().count());
        if argument_count + block_pass != 2 {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        ctx.report_with_fix(
            &Self::META,
            value.span(),
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(selector.span(), b"cast".to_vec())],
            },
        );
    }
}

/// `parent.assignment? && parent.children.last.equal?(node)`: the value of an
/// assignment node. A setter call (`self.foo = x`, `a[0] = x`) counts too, via
/// `MethodDispatchNode#assignment?` (`setter_method?`).
fn assigned_value<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => Some(node.as_local_variable_write_node()?.value()),
        NodeKind::InstanceVariableWriteNode => {
            Some(node.as_instance_variable_write_node()?.value())
        }
        NodeKind::ClassVariableWriteNode => Some(node.as_class_variable_write_node()?.value()),
        NodeKind::GlobalVariableWriteNode => Some(node.as_global_variable_write_node()?.value()),
        NodeKind::ConstantWriteNode => Some(node.as_constant_write_node()?.value()),
        NodeKind::ConstantPathWriteNode => Some(node.as_constant_path_write_node()?.value()),
        NodeKind::LocalVariableOrWriteNode => Some(node.as_local_variable_or_write_node()?.value()),
        NodeKind::LocalVariableAndWriteNode => {
            Some(node.as_local_variable_and_write_node()?.value())
        }
        NodeKind::LocalVariableOperatorWriteNode => {
            Some(node.as_local_variable_operator_write_node()?.value())
        }
        NodeKind::InstanceVariableOrWriteNode => {
            Some(node.as_instance_variable_or_write_node()?.value())
        }
        NodeKind::InstanceVariableAndWriteNode => {
            Some(node.as_instance_variable_and_write_node()?.value())
        }
        NodeKind::InstanceVariableOperatorWriteNode => {
            Some(node.as_instance_variable_operator_write_node()?.value())
        }
        NodeKind::ClassVariableOrWriteNode => Some(node.as_class_variable_or_write_node()?.value()),
        NodeKind::ClassVariableAndWriteNode => {
            Some(node.as_class_variable_and_write_node()?.value())
        }
        NodeKind::ClassVariableOperatorWriteNode => {
            Some(node.as_class_variable_operator_write_node()?.value())
        }
        NodeKind::GlobalVariableOrWriteNode => {
            Some(node.as_global_variable_or_write_node()?.value())
        }
        NodeKind::GlobalVariableAndWriteNode => {
            Some(node.as_global_variable_and_write_node()?.value())
        }
        NodeKind::GlobalVariableOperatorWriteNode => {
            Some(node.as_global_variable_operator_write_node()?.value())
        }
        NodeKind::ConstantOrWriteNode => Some(node.as_constant_or_write_node()?.value()),
        NodeKind::ConstantAndWriteNode => Some(node.as_constant_and_write_node()?.value()),
        NodeKind::ConstantOperatorWriteNode => {
            Some(node.as_constant_operator_write_node()?.value())
        }
        NodeKind::ConstantPathOrWriteNode => Some(node.as_constant_path_or_write_node()?.value()),
        NodeKind::ConstantPathAndWriteNode => Some(node.as_constant_path_and_write_node()?.value()),
        NodeKind::ConstantPathOperatorWriteNode => {
            Some(node.as_constant_path_operator_write_node()?.value())
        }
        NodeKind::CallOrWriteNode => Some(node.as_call_or_write_node()?.value()),
        NodeKind::IndexOrWriteNode => Some(node.as_index_or_write_node()?.value()),
        NodeKind::CallAndWriteNode => Some(node.as_call_and_write_node()?.value()),
        NodeKind::IndexAndWriteNode => Some(node.as_index_and_write_node()?.value()),
        NodeKind::CallOperatorWriteNode => Some(node.as_call_operator_write_node()?.value()),
        NodeKind::IndexOperatorWriteNode => Some(node.as_index_operator_write_node()?.value()),
        NodeKind::MultiWriteNode => Some(node.as_multi_write_node()?.value()),
        NodeKind::CallNode => {
            let call = node.as_call_node()?;
            if !call.is_attribute_write() {
                return None;
            }
            call.arguments()?.arguments().iter().last()
        }
        _ => None,
    }
}

fn is_const_t(node: &Node<'_>) -> bool {
    node.as_constant_read_node().is_some_and(|constant| constant.name().as_slice() == b"T")
}
