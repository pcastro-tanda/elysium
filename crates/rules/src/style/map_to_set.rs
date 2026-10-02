//! `Style/MapToSet`, ported from RuboCop's
//! `lib/rubocop/cop/style/map_to_set.rb`.
//!
//! See `map_to_hash.rs`'s module docs -- identical shape, except the
//! message has no `%<dot>s` placeholder (always reads `.to_set`
//! literally) and there is no minimum target Ruby version.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// Prefer `to_set` with a block over `map.to_set`.
#[derive(Debug, Clone)]
pub struct MapToSet;

impl Rule for MapToSet {
    const META: RuleMeta = RuleMeta {
        name: "Style/MapToSet",
        department: Department::Style,
        summary: "Prefer `to_set` with a block over `map.to_set`.",
        explanation: "",
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
        let Some(to_set) = node.as_call_node() else { return };
        if to_set.name().as_slice() != b"to_set" {
            return;
        }
        if to_set.block().is_some() || to_set.arguments().is_some() {
            return;
        }
        let Some(receiver) = to_set.receiver() else { return };
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
        let Some(to_set_dot) = to_set.call_operator_loc() else { return };
        let method_name = String::from_utf8_lossy(method);
        let message =
            format!("Pass a block to `to_set` instead of calling `{method_name}.to_set`.");

        let removal_range = Span::new(to_set_dot.span().start, selector_end(&to_set));
        let removal_range = ctx.with_surrounding_space(removal_range, Side::Left, true, false);

        let mut edits = vec![Edit::delete(removal_range)];
        edits.push(Edit::replace(selector.span(), b"to_set".to_vec()));

        ctx.report_with_fix(
            &Self::META,
            selector.span(),
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// `to_set.loc.selector.end_pos`.
fn selector_end(call: &CallNode<'_>) -> u32 {
    call.message_loc().map_or(call.as_node().span().end, |l| l.span().end)
}
