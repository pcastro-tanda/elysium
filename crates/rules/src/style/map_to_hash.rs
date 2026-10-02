//! `Style/MapToHash`, ported from RuboCop's
//! `lib/rubocop/cop/style/map_to_hash.rb`.
//!
//! See `map_join.rs`'s module docs for the general shape translation: a
//! `map`/`collect` `CallNode`'s own `block` field is either a literal
//! `BlockNode` (upstream's `any_block` pattern, any parameters/body) or a
//! `&:sym` block-pass (`Some(Node::BlockArgumentNode)` wrapping a
//! `SymbolNode`, any symbol, unlike `Style/MapJoin`'s `:to_s`-only check).
//!
//! `destructuring_argument` (`|(k, v)|` stripped to `|k, v|` when `to_h`'s
//! block replaces `map`'s own) is Prism's `MultiTargetNode`: the *sole*
//! entry in `BlockParametersNode::parameters().requireds()`, with every
//! other parameter slot empty, parenthesised via its own `lparen_loc`/
//! `rparen_loc`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// Prefer `to_h` with a block over `map.to_h`.
#[derive(Debug, Clone)]
pub struct MapToHash {
    target_ruby_version: f32,
}

impl Rule for MapToHash {
    const META: RuleMeta = RuleMeta {
        name: "Style/MapToHash",
        department: Department::Style,
        summary: "Prefer `to_h` with a block over `map.to_h`.",
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
        if self.target_ruby_version < 2.6 {
            return;
        }
        let Some(to_h) = node.as_call_node() else { return };
        if to_h.name().as_slice() != b"to_h" {
            return;
        }
        // `return if to_h_node.block_literal?`
        if to_h.block().is_some_and(|b| b.as_block_node().is_some()) {
            return;
        }
        let Some(receiver) = to_h.receiver() else { return };
        let Some(map_call) = receiver.as_call_node() else { return };
        let method = map_call.name();
        let method = method.as_slice();
        if method != b"map" && method != b"collect" {
            return;
        }
        let has_literal_block = map_call.block().is_some_and(|b| b.as_block_node().is_some());
        let has_symbol_block_pass = map_call.block().is_some_and(|b| {
            b.as_block_argument_node()
                .is_some_and(|ba| ba.expression().is_some_and(|e| e.as_symbol_node().is_some()))
        });
        if !has_literal_block && !has_symbol_block_pass {
            return;
        }

        let Some(selector) = map_call.message_loc() else { return };
        let Some(to_h_dot) = to_h.call_operator_loc() else { return };
        let method_name = String::from_utf8_lossy(method);
        let dot_text = String::from_utf8_lossy(to_h_dot.as_slice());
        let message =
            format!("Pass a block to `to_h` instead of calling `{method_name}{dot_text}to_h`.");

        let removal_range = Span::new(to_h_dot.span().start, selector_end(&to_h));
        let removal_range = ctx.with_surrounding_space(removal_range, Side::Left, true, false);

        let mut edits = vec![Edit::delete(removal_range)];
        if let Some(map_dot) = map_call.call_operator_loc() {
            edits.push(Edit::replace(map_dot.span(), to_h_dot.as_slice().to_vec()));
        }
        edits.push(Edit::replace(selector.span(), b"to_h".to_vec()));

        if has_literal_block {
            if let Some(arg_span) = destructuring_argument(&map_call) {
                let text = ctx.text(arg_span);
                edits.push(Edit::replace(arg_span, text[1..text.len() - 1].to_vec()));
            }
        }

        ctx.report_with_fix(
            &Self::META,
            selector.span(),
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// `to_h.loc.selector.end_pos`.
fn selector_end(call: &CallNode<'_>) -> u32 {
    call.message_loc().map_or(call.as_node().span().end, |l| l.span().end)
}

/// RuboCop's `destructuring_argument`: the block's sole parameter is a
/// parenthesised multi-target (`|(k, v)|`), with no other parameter slot in
/// use.
fn destructuring_argument(map_call: &CallNode<'_>) -> Option<Span> {
    let block = map_call.block().and_then(|b| b.as_block_node())?;
    let params = block.parameters().and_then(|p| p.as_block_parameters_node())?;
    let parameters = params.parameters()?;
    if parameters.requireds().len() != 1
        || !parameters.optionals().is_empty()
        || parameters.rest().is_some()
        || !parameters.posts().is_empty()
        || !parameters.keywords().is_empty()
        || parameters.keyword_rest().is_some()
    {
        return None;
    }
    let sole = parameters.requireds().first()?;
    sole.as_multi_target_node().map(|m| m.as_node().span())
}
