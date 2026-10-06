//! `Rails/SelectMap`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/select_map.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{each_descendant, for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks for uses of `select(:column_name)` with `map(&:column_name)`.
#[derive(Debug, Clone)]
pub struct SelectMap;

impl Rule for SelectMap {
    const META: RuleMeta = RuleMeta {
        name: "Rails/SelectMap",
        department: Department::Rails,
        summary: "Checks for uses of `select(:column_name)` with `map(&:column_name)`.",
        explanation: "Checks for uses of `select(:column_name)` with `map(&:column_name)`.\n\
                      These can be replaced with `pluck(:column_name)`.\n\nThere also should be \
                      some performance improvement since it skips instantiating the model class \
                      for matches.\n\nThis cop is unsafe because the model might override the \
                      attribute getter. Additionally, the model's `after_initialize` hooks are \
                      skipped when using `pluck`.\n\n```ruby\n# bad\n\
                      Model.select(:column_name).map(&:column_name)\n\n# good\n\
                      Model.pluck(:column_name)\n```",
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
        if !matches!(call.name().as_slice(), b"map" | b"collect") {
            return;
        }
        let Some(first_argument) = first_argument_span(node) else { return };
        let source = ctx.text(first_argument);
        let column_name = source.strip_prefix(b"&:").unwrap_or(source).to_vec();
        let Some((select, select_parent_dot)) = find_select_node(node, &column_name, ctx) else {
            return;
        };

        let node_end = call_end(node);
        let offense = Span::new(select.selector_start, node_end);
        let mut preferred_method = b"pluck(:".to_vec();
        preferred_method.extend_from_slice(&column_name);
        preferred_method.push(b')');
        let map_method = String::from_utf8_lossy(call.name().as_slice()).into_owned();
        let message = format!(
            "Use `{}` instead of `select` with `{map_method}`.",
            String::from_utf8_lossy(&preferred_method)
        );

        let edits = if let Some(dot) = select_parent_dot {
            let selector = call.message_loc().map_or(offense.start, |loc| loc.span().start);
            vec![
                Edit::delete(dot),
                Edit::delete(Span::new(select.selector_start, select.end)),
                Edit::replace(Span::new(selector, node_end), preferred_method),
            ]
        } else {
            let Some(receiver) = select.receiver else {
                // Upstream raises `NoMethodError` on a receiverless `select` here.
                return;
            };
            let mut replacement = ctx.text(receiver).to_vec();
            replacement.push(b'.');
            replacement.extend_from_slice(&preferred_method);
            vec![Edit::replace(Span::new(node.span().start, node_end), replacement)]
        };
        ctx.report_with_fix(
            &Self::META,
            offense,
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// A `select` call found in the receiver chain.
struct SelectNode {
    selector_start: u32,
    end: u32,
    /// `select_node.receiver`.
    receiver: Option<Span>,
}

/// End of the `send` node: a literal block is not part of it, a block-pass is.
fn call_end(node: &Node<'_>) -> u32 {
    let Some(call) = node.as_call_node() else { return node.span().end };
    match call.block() {
        Some(block) if block.as_block_node().is_none() => node.span().end,
        _ => call_span_excluding_block(&call).end,
    }
}

/// `node.first_argument`: the first argument, which for whitequark includes
/// a block-pass.
fn first_argument_span(node: &Node<'_>) -> Option<Span> {
    let call = node.as_call_node()?;
    if let Some(arguments) = call.arguments() {
        if let Some(first) = arguments.arguments().iter().next() {
            return Some(first.span());
        }
    }
    let block = call.block()?;
    block.as_block_argument_node().map(|_| block.span())
}

/// `find_select_node`: the single `select(column)` among the descendants, if
/// it sits in the receiver chain. Also yields the parent's dot when the
/// parent is a call with one.
fn find_select_node(
    node: &Node<'_>,
    column_name: &[u8],
    ctx: &Context<'_>,
) -> Option<(SelectNode, Option<Span>)> {
    let mut matches: Vec<Span> = Vec::new();
    let mut visit = |candidate: &Node<'_>| {
        if is_select_with_column(candidate, column_name, ctx) {
            matches.push(candidate.span());
        }
    };
    // A literal block belongs to the `block` node in whitequark, not to the
    // `send`.
    for_each_child(node, |child| {
        if child.as_block_node().is_some() {
            return;
        }
        visit(child);
        each_descendant(child, &mut visit);
    });
    let [select_span] = matches.as_slice() else { return None };

    // `receiver_chain?`
    let call = node.as_call_node()?;
    let mut current = call.receiver();
    // The dot of the call whose receiver is `current`, if that parent is a call.
    let mut parent_dot = call.call_operator_loc().map(|loc| loc.span());
    while let Some(mut candidate) = current {
        if let Some(parens) = candidate.as_parentheses_node() {
            let body = parens.body()?;
            candidate = body.as_statements_node()?.body().iter().last()?;
            parent_dot = None;
        }
        let call = candidate.as_call_node()?;
        // A call with a literal block is a `block` node: neither the `select`
        // send nor a call, so the chain ends there.
        if call.block().is_some_and(|block| block.as_block_node().is_some()) {
            return None;
        }
        if candidate.span() == *select_span {
            return Some((
                SelectNode {
                    selector_start: call.message_loc()?.span().start,
                    end: call_end(&candidate),
                    receiver: call.receiver().map(|receiver| receiver.span()),
                },
                parent_dot,
            ));
        }
        parent_dot = call.call_operator_loc().map(|loc| loc.span());
        current = call.receiver();
    }
    None
}

fn is_select_with_column(candidate: &Node<'_>, column_name: &[u8], ctx: &Context<'_>) -> bool {
    let Some(call) = candidate.as_call_node() else { return false };
    if call.name().as_slice() != b"select" {
        return false;
    }
    // `arguments.one?`: a block-pass counts as an argument in whitequark.
    let Some(arguments) = call.arguments() else { return false };
    let mut iter = arguments.arguments().iter();
    let (Some(first), None) = (iter.next(), iter.next()) else { return false };
    if call.block().is_some_and(|block| block.as_block_argument_node().is_some()) {
        return false;
    }
    let argument: Option<Vec<u8>> = if first.as_symbol_node().is_some() {
        let source = ctx.text(first.span());
        Some(source.strip_prefix(b":").unwrap_or(source).to_vec())
    } else {
        first.as_string_node().map(|string| string.unescaped().to_vec())
    };
    argument.as_deref() == Some(column_name)
}
