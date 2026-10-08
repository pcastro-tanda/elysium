//! `Performance/MethodObjectAsBlock`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/method_object_as_block.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Use block explicitly instead of block-passing a method object.";

/// Use block explicitly instead of block-passing a method object.
#[derive(Debug, Clone)]
pub struct MethodObjectAsBlock;

impl Rule for MethodObjectAsBlock {
    const META: RuleMeta = RuleMeta {
        name: "Performance/MethodObjectAsBlock",
        department: Department::Performance,
        summary: "Use block explicitly instead of block-passing a method object.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // `(^send (send _ :method sym))`: the parent must be a plain `send`.
        if call.is_safe_navigation() {
            return;
        }
        let Some(block) = call.block() else { return };
        let Some(block_pass) = block.as_block_argument_node() else { return };
        let Some(inner) = block_pass.expression().and_then(|e| e.as_call_node()) else { return };
        if inner.is_safe_navigation()
            || inner.name().as_slice() != b"method"
            || inner.block().is_some()
        {
            return;
        }
        let Some(arguments) = inner.arguments() else { return };
        let mut it = arguments.arguments().iter();
        let (Some(arg), None) = (it.next(), it.next()) else { return };
        if arg.as_symbol_node().is_some() {
            ctx.report(&Self::META, block.span(), MSG);
        }
    }
}
