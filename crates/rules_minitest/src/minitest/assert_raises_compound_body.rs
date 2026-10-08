//! `Minitest/AssertRaisesCompoundBody`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_raises_compound_body.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Reduce `assert_raises` block body to contain only the raising code.";

/// This cop enforces the block body of `assert_raises { ... }` to be reduced to only the raising code.
#[derive(Debug, Clone)]
pub struct AssertRaisesCompoundBody;

impl Rule for AssertRaisesCompoundBody {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertRaisesCompoundBody",
        department: Department::Minitest,
        summary: "This cop enforces the block body of `assert_raises { ... }` to be reduced to only the raising code.",
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
        // `on_block` only: numbered-parameter and `it` blocks are other node types.
        let Some(block) = call.block().and_then(|block| block.as_block_node()) else { return };
        if block.parameters().is_some_and(|parameters| {
            parameters.as_numbered_parameters_node().is_some()
                || parameters.as_it_parameters_node().is_some()
        }) {
            return;
        }
        if call.name().as_slice() != b"assert_raises" {
            return;
        }
        if multi_statement_begin(block.body()) {
            ctx.report(&Self::META, node.span(), MSG);
        }
    }
}

/// `node&.begin_type? && node.children.size > 1`.
fn multi_statement_begin(body: Option<Node<'_>>) -> bool {
    let Some(body) = body else { return false };
    let Some(statements) = body.as_statements_node() else { return false };
    let list: Vec<Node<'_>> = statements.body().iter().collect();
    match list.as_slice() {
        [] => false,
        [single] => single
            .as_parentheses_node()
            .and_then(|parens| parens.body())
            .and_then(|inner| inner.as_statements_node())
            .is_some_and(|inner| inner.body().iter().count() > 1),
        _ => true,
    }
}
