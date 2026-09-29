//! `Style/KeywordParametersOrder`, ported from RuboCop's
//! `lib/rubocop/cop/style/keyword_parameters_order.rb`.
//!
//! Prism represents each keyword parameter as its own typed node
//! (`RequiredKeywordParameterNode` / `OptionalKeywordParameterNode`) inside
//! `ParametersNode#keywords`, in source order -- no `right_siblings` walk
//! over a flat `args` child list is needed like in whitequark. This rule
//! instead visits the owning `DefNode`/`BlockNode`/`LambdaNode` directly, so
//! it has the paren/pipe context and the full parameter list (needed for the
//! `append_newline_to_last_kwoptarg` blank-line fixup) in hand without a
//! separate `each_ancestor(:any_def, :block)` lookup.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::ParametersNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind, NodeList};
use ruby_source::{Side, Span};

const MSG: &str = "Place optional keyword parameters at the end of the parameters list.";

/// Enforces that optional keyword parameters are placed at the end of the parameters list.
#[derive(Debug, Clone)]
pub struct KeywordParametersOrder;

impl Rule for KeywordParametersOrder {
    const META: RuleMeta = RuleMeta {
        name: "Style/KeywordParametersOrder",
        department: Department::Style,
        summary: "Enforces that optional keyword parameters are placed at the end of the parameters list.",
        explanation: "\
Enforces that optional keyword parameters are placed at the end of the
parameters list.

This improves readability, because when looking through the source, it is
expected to find required parameters at the beginning of the parameters
list and optional parameters at the end.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode, NodeKind::BlockNode, NodeKind::LambdaNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(owner) = Owner::from(node) else { return };
        let Some(parameters) = owner.parameters else { return };
        check_parameters(&parameters, owner.parenthesized, owner.restore_newline, ctx);
    }
}

/// The def/block/lambda owning a parameter list, reduced to what
/// `autocorrect`/`append_newline_to_last_kwoptarg` need: its parameters (if
/// any), whether they are parenthesized (`parentheses?`), and whether a
/// removed trailing `kwoptarg`'s line break may need restoring. Upstream
/// skips the restore for `arguments.parent.block_type?`, which whitequark
/// reports `true` for both classic blocks and `-> {}` lambdas -- Prism's
/// `BlockNode`/`LambdaNode` here.
struct Owner<'pr> {
    parameters: Option<ParametersNode<'pr>>,
    parenthesized: bool,
    restore_newline: bool,
}

impl<'pr> Owner<'pr> {
    fn from(node: &Node<'pr>) -> Option<Self> {
        match node.kind() {
            NodeKind::DefNode => {
                let def = node.as_def_node().expect("kind matched");
                Some(Self {
                    parameters: def.parameters(),
                    parenthesized: def.lparen_loc().is_some(),
                    restore_newline: true,
                })
            }
            NodeKind::BlockNode => {
                let block = node.as_block_node().expect("kind matched");
                let parameters = block
                    .parameters()
                    .and_then(|p| p.as_block_parameters_node())
                    .and_then(|p| p.parameters());
                Some(Self { parameters, parenthesized: false, restore_newline: false })
            }
            NodeKind::LambdaNode => {
                let lambda = node.as_lambda_node().expect("kind matched");
                let parameters = lambda.parameters().and_then(|p| p.as_parameters_node());
                Some(Self { parameters, parenthesized: true, restore_newline: false })
            }
            _ => None,
        }
    }
}

/// RuboCop's `on_kwoptarg`, driven from the parameter list as a whole
/// instead of once per `kwoptarg` node.
fn check_parameters(
    parameters: &ParametersNode<'_>,
    parenthesized: bool,
    restore_newline: bool,
    ctx: &mut Context<'_>,
) {
    let keywords: Vec<Node<'_>> = parameters.keywords().iter().collect();
    let first_kwoptarg =
        keywords.iter().position(|n| n.kind() == NodeKind::OptionalKeywordParameterNode);

    for (i, node) in keywords.iter().enumerate() {
        if node.kind() != NodeKind::OptionalKeywordParameterNode {
            continue;
        }
        let kwarg_nodes: Vec<Node<'_>> = keywords[i + 1..]
            .iter()
            .filter(|n| n.kind() == NodeKind::RequiredKeywordParameterNode)
            .copied()
            .collect();
        if kwarg_nodes.is_empty() {
            continue;
        }

        let span = node.span();

        // `node.parent.find(&:kwoptarg_type?) == node`: only the first
        // `kwoptarg` in the parameter list performs the actual move: the
        // later ones' correction blocks bail out immediately upstream too.
        if first_kwoptarg != Some(i) {
            ctx.report(&KeywordParametersOrder::META, span, MSG);
            continue;
        }

        // `processed_source.contains_comment?(arguments_range(defining_node))`
        if comment_in_parameters(ctx, parameters) {
            ctx.report(&KeywordParametersOrder::META, span, MSG);
            continue;
        }

        let fix = build_fix(
            ctx,
            parameters,
            &keywords,
            *node,
            &kwarg_nodes,
            parenthesized,
            restore_newline,
        );
        ctx.report_with_fix(&KeywordParametersOrder::META, span, MSG, fix);
    }
}

/// `arguments_range(defining_node)` (the whole parameter list, any kind) with
/// `processed_source.contains_comment?`.
fn comment_in_parameters(ctx: &Context<'_>, parameters: &ParametersNode<'_>) -> bool {
    let range = parameters.location().span();
    ctx.comments().iter().any(|c| c.span.start < range.end && c.span.end > range.start)
}

/// `autocorrect`: inserts the moved `kwarg`s' source before the leading
/// `kwoptarg`, restores a lost trailing newline when needed, then removes
/// each moved `kwarg` (with its surrounding space and a now-adjacent comma).
fn build_fix(
    ctx: &Context<'_>,
    parameters: &ParametersNode<'_>,
    keywords: &[Node<'_>],
    node: Node<'_>,
    kwarg_nodes: &[Node<'_>],
    parenthesized: bool,
    restore_newline: bool,
) -> Fix {
    let mut edits = Vec::with_capacity(kwarg_nodes.len() + 2);

    let mut prefix = Vec::new();
    for (i, kwarg) in kwarg_nodes.iter().enumerate() {
        if i > 0 {
            prefix.extend_from_slice(b", ");
        }
        prefix.extend_from_slice(ctx.text(kwarg.span()));
    }
    prefix.extend_from_slice(b", ");
    edits.push(Edit::insert(node.span().start, prefix));

    if restore_newline
        && !parenthesized
        && overall_last_parameter(parameters)
            .is_some_and(|n| n.kind() == NodeKind::RequiredKeywordParameterNode)
    {
        if let Some(last_kwoptarg) =
            keywords.iter().rev().find(|n| n.kind() == NodeKind::OptionalKeywordParameterNode)
        {
            edits.push(Edit::insert(last_kwoptarg.span().end, b"\n".to_vec()));
        }
    }

    for kwarg in kwarg_nodes {
        edits.push(Edit::delete(removal_span(ctx, kwarg.span())));
    }

    Fix { applicability: Applicability::Safe, edits }
}

/// The parameter list's actual last parameter in Ruby's fixed grammar order
/// (required, optional, rest, post, keywords, kwrest, block) -- `kwarg_type?`
/// on it mirrors upstream's `arguments.last.kwarg_type?`.
fn overall_last_parameter<'pr>(parameters: &ParametersNode<'pr>) -> Option<Node<'pr>> {
    if let Some(block) = parameters.block() {
        return Some(block.as_node());
    }
    if let Some(keyword_rest) = parameters.keyword_rest() {
        return Some(keyword_rest);
    }
    if let Some(last) = last_of(&parameters.keywords()) {
        return Some(last);
    }
    if let Some(last) = last_of(&parameters.posts()) {
        return Some(last);
    }
    if let Some(rest) = parameters.rest() {
        return Some(rest);
    }
    if let Some(last) = last_of(&parameters.optionals()) {
        return Some(last);
    }
    last_of(&parameters.requireds())
}

fn last_of<'pr>(list: &NodeList<'pr>) -> Option<Node<'pr>> {
    list.iter().last()
}

/// `range_with_surrounding_space(kwarg.source_range)` (both sides, crossing a
/// single trailing newline) then `range_with_surrounding_comma(range, :left)`.
fn removal_span(ctx: &Context<'_>, node_span: Span) -> Span {
    let expanded = ctx.with_surrounding_space(node_span, Side::Both, true, false);
    let bytes = ctx.source().bytes();
    let mut start = expanded.start;
    while start > 0 && bytes[(start - 1) as usize] == b',' {
        start -= 1;
    }
    Span::new(start, expanded.end)
}
