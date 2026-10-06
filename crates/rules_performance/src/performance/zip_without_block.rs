//! `Performance/ZipWithoutBlock`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/zip_without_block.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `zip` without a block argument instead.";

/// Checks for `map { |id| [id] }` and suggests replacing it with `zip`.
#[derive(Debug, Clone)]
pub struct ZipWithoutBlock;

impl Rule for ZipWithoutBlock {
    const META: RuleMeta = RuleMeta {
        name: "Performance/ZipWithoutBlock",
        department: Department::Performance,
        summary: "Checks for `map { |id| [id] }` and suggests replacing it with `zip`.",
        explanation: "Checks for `map { |id| [id] }` and suggests replacing it with `zip`.\n\n\
                      This cop is unsafe for novel definitions of `map` and `collect` on \
                      non-Enumerable objects that do not respond to `zip`.\n\n```ruby\n# bad\n\
                      [1, 2, 3].map { |id| [id] }\n\n# good\n[1, 2, 3].zip\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
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
        if !matches!(call.name().as_slice(), b"map" | b"collect")
            || call.receiver().is_none()
            || call.arguments().is_some()
        {
            return;
        }
        let Some(block_node) = call.block() else { return };
        let Some(block) = block_node.as_block_node() else { return };
        if !map_with_array(&block) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let span = Span::new(selector.span().start, block.closing_loc().span().end);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, b"zip".to_vec())],
            },
        );
    }
}

/// `(block _ (args (arg _)) (array (lvar _)))`, `(numblock _ 1 (array (lvar _)))`
/// and `(itblock _ :it (array (lvar _)))`.
fn map_with_array(block: &ruby_ast::node::BlockNode<'_>) -> bool {
    let params_ok = match block.parameters() {
        Some(p) => {
            if let Some(bp) = p.as_block_parameters_node() {
                bp.locals().is_empty()
                    && bp.parameters().is_some_and(|ps| {
                        ps.requireds().len() == 1
                            && ps
                                .requireds()
                                .first()
                                .is_some_and(|r| r.as_required_parameter_node().is_some())
                            && ps.optionals().is_empty()
                            && ps.rest().is_none()
                            && ps.posts().is_empty()
                            && ps.keywords().is_empty()
                            && ps.keyword_rest().is_none()
                            && ps.block().is_none()
                    })
            } else if let Some(n) = p.as_numbered_parameters_node() {
                n.maximum() == 1
            } else {
                p.as_it_parameters_node().is_some()
            }
        }
        None => false,
    };
    if !params_ok {
        return false;
    }
    let Some(body) = block.body() else { return false };
    let Some(statements) = body.as_statements_node() else { return false };
    if statements.body().len() != 1 {
        return false;
    }
    let Some(array) = statements.body().first().and_then(|s| s.as_array_node()) else {
        return false;
    };
    array.elements().len() == 1
        && array.elements().first().is_some_and(|e| {
            e.as_local_variable_read_node().is_some()
                || e.as_it_local_variable_read_node().is_some()
        })
}
