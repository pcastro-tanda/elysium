//! `Sorbet/TypeAliasName`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/type_alias_name.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Type alias constant name should be in CamelCase";

/// Type alias constant names must be in CamelCase.
#[derive(Debug, Clone)]
pub struct TypeAliasName;

impl Rule for TypeAliasName {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/TypeAliasName",
        department: Department::Sorbet,
        summary: "Type alias constant names must be in CamelCase.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ConstantWriteNode, NodeKind::ConstantPathWriteNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (name, value) = if let Some(write) = node.as_constant_write_node() {
            (write.name().as_slice().to_vec(), write.value())
        } else if let Some(write) = node.as_constant_path_write_node() {
            let Some(name) = write.target().name() else { return };
            (name.as_slice().to_vec(), write.value())
        } else {
            return;
        };
        if name.contains(&b'_') && is_t_type_alias_block(&value) {
            ctx.report(&Self::META, node.span(), MSG);
        }
    }
}

/// `(block (send (const nil? :T) :type_alias) ...)`.
fn is_t_type_alias_block(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return false };
    if block.parameters().is_some_and(|p| p.as_block_parameters_node().is_none()) {
        return false;
    }
    !call.is_safe_navigation()
        && call.name().as_slice() == b"type_alias"
        && call
            .receiver()
            .and_then(|r| r.as_constant_read_node().map(|c| c.name().as_slice() == b"T"))
            .unwrap_or(false)
}
