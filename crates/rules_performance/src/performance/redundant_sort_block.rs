//! `Performance/RedundantSortBlock`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/redundant_sort_block.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::BlockNode;
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `sort` without block.";

/// Use `sort` instead of `sort { |a, b| a <=> b }`.
#[derive(Debug, Clone)]
pub struct RedundantSortBlock;

impl Rule for RedundantSortBlock {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RedundantSortBlock",
        department: Department::Performance,
        summary: "Use `sort` instead of `sort { |a, b| a <=> b }`.",
        explanation: "",
        enabled_by_default: false,
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
        if call.name().as_slice() != b"sort" || call.arguments().is_some() {
            return;
        }
        let Some(block) = call.block().and_then(|block| block.as_block_node()) else { return };
        let Some(selector) = call.message_loc() else { return };
        let Some(params) = block.parameters() else { return };

        if let Some(block_params) = params.as_block_parameters_node() {
            if block_params.locals().iter().next().is_some() {
                return;
            }
            let Some(parameters) = block_params.parameters() else { return };
            if parameters.optionals().iter().next().is_some()
                || parameters.rest().is_some()
                || parameters.posts().iter().next().is_some()
                || parameters.keywords().iter().next().is_some()
                || parameters.keyword_rest().is_some()
                || parameters.block().is_some()
            {
                return;
            }
            let requireds: Vec<Node<'_>> = parameters.requireds().iter().collect();
            let [a, b] = requireds.as_slice() else { return };
            let (Some(a), Some(b)) =
                (a.as_required_parameter_node(), b.as_required_parameter_node())
            else {
                return;
            };
            if !body_replaceable(&block, a.name().as_slice(), b.name().as_slice()) {
                return;
            }
        } else if params.as_numbered_parameters_node().is_some_and(|n| n.maximum() == 2) {
            if !body_replaceable(&block, b"_1", b"_2") {
                return;
            }
        } else {
            return;
        }

        let span = Span::new(selector.span().start, block.closing_loc().span().end);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, b"sort".to_vec())],
            },
        );
    }
}

/// `(send (lvar %1) :<=> (lvar %2))` as the block's whole body.
fn body_replaceable(block: &BlockNode<'_>, a: &[u8], b: &[u8]) -> bool {
    let Some(body) = block.body() else { return false };
    let Some(statements) = body.as_statements_node() else { return false };
    let stmts: Vec<Node<'_>> = statements.body().iter().collect();
    let [stmt] = stmts.as_slice() else { return false };
    let Some(cmp) = stmt.as_call_node() else { return false };
    if cmp.is_safe_navigation() || cmp.name().as_slice() != b"<=>" || cmp.block().is_some() {
        return false;
    }
    let Some(receiver) = cmp.receiver() else { return false };
    let args: Vec<Node<'_>> =
        cmp.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
    let [arg] = args.as_slice() else { return false };
    let is_lvar = |node: &Node<'_>, name: &[u8]| {
        node.as_local_variable_read_node().is_some_and(|v| v.name().as_slice() == name)
    };
    is_lvar(&receiver, a) && is_lvar(arg, b)
}
