//! `Performance/MapCompact`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/map_compact.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `filter_map` instead.";

/// Use `filter_map` instead of `collection.map(&:do_something).compact`.
#[derive(Debug, Clone)]
pub struct MapCompact {
    target_ruby_version: f32,
}

impl Rule for MapCompact {
    const META: RuleMeta = RuleMeta {
        name: "Performance/MapCompact",
        department: Department::Performance,
        summary: "Use `filter_map` instead of `collection.map(&:do_something).compact`.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.target_ruby_version < 2.7 {
            return;
        }
        let Some(compact) = node.as_call_node() else { return };
        if compact.name().as_slice() != b"compact" || compact.arguments().is_some() {
            return;
        }
        let own_block = match compact.block() {
            None => false,
            Some(b) if b.as_block_node().is_some() => true,
            Some(_) => return,
        };
        let Some(map_node) = compact.receiver().and_then(|r| r.as_call_node()) else { return };
        let map_name = map_node.name();
        if (map_name.as_slice() != b"map" && map_name.as_slice() != b"collect")
            || map_node.arguments().is_some()
        {
            return;
        }
        let matches = match map_node.block() {
            // `(block_pass (sym _))`
            Some(b) => {
                if let Some(bp) = b.as_block_argument_node() {
                    bp.expression().is_some_and(|e| e.as_symbol_node().is_some())
                } else if let Some(block) = b.as_block_node() {
                    // `(args ...)`: not a numbered-parameter or `it` block.
                    block.parameters().is_none_or(|p| p.as_block_parameters_node().is_some())
                } else {
                    false
                }
            }
            None => false,
        };
        if !matches {
            return;
        }
        let (Some(map_sel), Some(compact_sel)) = (map_node.message_loc(), compact.message_loc())
        else {
            return;
        };
        let (map_sel, compact_sel) = (map_sel.span(), compact_sel.span());
        let Some(dot) = compact.call_operator_loc() else { return };
        let dot = dot.span();

        let mut edits = vec![Edit::replace(map_sel, b"filter_map".to_vec())];
        let compact_span = call_span_excluding_block(&compact);
        let chained = if own_block { None } else { chained_method_info(ctx) };
        let multiline_removal = !ctx.is_single_line(compact_span)
            && chained.is_some_and(|(use_dot, last_line)| {
                let compact_line = ctx.line_col(compact_sel.start).line;
                use_dot
                    && compact_line != ctx.line_col(map_sel.start).line
                    && compact_line != last_line
            });
        if multiline_removal {
            edits.push(Edit::delete(ctx.whole_lines(compact_sel)));
        } else {
            edits.push(Edit::delete(dot));
            edits.push(Edit::delete(compact_sel));
        }
        ctx.report_with_fix(
            &Self::META,
            Span::new(map_sel.start, compact_sel.end),
            MSG,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// For the parent send of the node being entered (skipping the `ArgumentsNode`
/// Prism adds between a call and its arguments): `(has a selector and uses a
/// plain '.', last line of the send excluding its block)`. `None` when the
/// parent is not a call with a selector.
fn chained_method_info(ctx: &Context<'_>) -> Option<(bool, u32)> {
    let parent = ctx
        .ancestors()
        .iter()
        .rev()
        .find(|a| a.kind != NodeKind::ArgumentsNode)
        .filter(|a| a.kind == NodeKind::CallNode)?;
    let root = ctx.parsed().root();
    let mut found = None;
    each_descendant(&root, &mut |n: &Node<'_>| {
        if found.is_none() && n.kind() == NodeKind::CallNode && n.span() == parent.span {
            found = Some(n.as_call_node().expect("kind checked"));
        }
    });
    let call = found?;
    call.message_loc()?;
    let use_dot = call.call_operator_loc().is_some_and(|l| ctx.text(l.span()) == b".");
    let end = if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
        call.as_node().span()
    } else {
        call_span_excluding_block(&call)
    };
    Some((use_dot, ctx.last_line(end)))
}
