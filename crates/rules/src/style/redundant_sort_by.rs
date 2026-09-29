//! `Style/RedundantSortBy`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_sort_by.rb`.
//!
//! Upstream matches on `on_block`/`on_numblock`/`on_itblock` a `sort_by`
//! call whose block body is nothing but a read of its own single
//! parameter (`(block $(call _ :sort_by) (args (arg $_x)) (lvar _x))`,
//! and the numbered-/`it`-parameter equivalents). In Prism the call is
//! visited instead (`CallNode::block()` points *to* the `BlockNode`,
//! rather than whitequark's `block` node wrapping the call), so this
//! rule subscribes to `CallNode` and inspects its attached block.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, StatementsNode};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MSG_NUMBLOCK: &str = "Use `sort` instead of `sort_by { _1 }`.";
const MSG_ITBLOCK: &str = "Use `sort` instead of `sort_by { it }`.";

/// Use `sort` instead of `sort_by { |x| x }`.
#[derive(Debug, Clone)]
pub struct RedundantSortBy;

impl Rule for RedundantSortBy {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantSortBy",
        department: Department::Style,
        summary: "Use `sort` instead of `sort_by { |x| x }`.",
        explanation: "Identifies places where `sort_by { ... }` can be replaced by \
            `sort`.\n\n\
            ```ruby\n\
            # bad\n\
            array.sort_by { |x| x }\n\
            array.sort_by do |var|\n\
              var\n\
            end\n\n\
            # good\n\
            array.sort\n\
            ```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
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
        if call.name().as_slice() != b"sort_by" {
            return;
        }
        let Some(block_node) = call.block() else { return };
        let Some(block) = block_node.as_block_node() else { return };
        let Some(message_loc) = call.message_loc() else { return };

        let message = if let Some(var_name) = redundant_var_name(&block) {
            format!(
                "Use `sort` instead of `sort_by {{ |{v}| {v} }}`.",
                v = String::from_utf8_lossy(&var_name)
            )
        } else if is_redundant_numblock(&block) {
            MSG_NUMBLOCK.to_string()
        } else if is_redundant_itblock(&block) {
            MSG_ITBLOCK.to_string()
        } else {
            return;
        };

        let span = Span::new(message_loc.span().start, block.location().span().end);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, b"sort".to_vec())],
            },
        );
    }
}

/// A `StatementsNode` with exactly one statement, standing in for that
/// statement itself.
fn single_statement<'pr>(stmts: &StatementsNode<'pr>) -> Option<Node<'pr>> {
    let body = stmts.body();
    (body.len() == 1).then(|| body.first()).flatten()
}

/// `(args (arg $_x)) (lvar _x)`: a block taking exactly one plain
/// required parameter (no optionals/rest/posts/keywords/block-arg, and no
/// shadow `; locals`) whose single-statement body reads that same
/// variable back. Returns the parameter's name.
fn redundant_var_name(block: &BlockNode<'_>) -> Option<Vec<u8>> {
    let params = block.parameters()?;
    let block_params = params.as_block_parameters_node()?;
    if !block_params.locals().is_empty() {
        return None;
    }
    let inner = block_params.parameters()?;
    if !inner.optionals().is_empty()
        || inner.rest().is_some()
        || !inner.posts().is_empty()
        || !inner.keywords().is_empty()
        || inner.keyword_rest().is_some()
        || inner.block().is_some()
    {
        return None;
    }
    let requireds: Vec<Node<'_>> = inner.requireds().iter().collect();
    let [only] = requireds.as_slice() else { return None };
    let name = only.as_required_parameter_node()?.name().as_slice().to_vec();

    let stmts = block.body()?.as_statements_node()?;
    let stmt = single_statement(&stmts)?;
    let lvar = stmt.as_local_variable_read_node()?;
    (lvar.name().as_slice() == name.as_slice()).then_some(name)
}

/// `(numblock $(call _ :sort_by) 1 (lvar :_1))`.
fn is_redundant_numblock(block: &BlockNode<'_>) -> bool {
    let Some(params) = block.parameters() else { return false };
    let Some(np) = params.as_numbered_parameters_node() else { return false };
    if np.maximum() != 1 {
        return false;
    }
    let Some(stmts) = block.body().and_then(|b| b.as_statements_node()) else { return false };
    let Some(stmt) = single_statement(&stmts) else { return false };
    stmt.as_local_variable_read_node().is_some_and(|lv| lv.name().as_slice() == b"_1")
}

/// `(itblock $(call _ :sort_by) _ (lvar :it))`.
fn is_redundant_itblock(block: &BlockNode<'_>) -> bool {
    let Some(params) = block.parameters() else { return false };
    if params.as_it_parameters_node().is_none() {
        return false;
    }
    let Some(stmts) = block.body().and_then(|b| b.as_statements_node()) else { return false };
    let Some(stmt) = single_statement(&stmts) else { return false };
    stmt.as_it_local_variable_read_node().is_some()
}
