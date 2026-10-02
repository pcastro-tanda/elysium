//! `Style/ArrayIntersectWithSingleElement`, ported from RuboCop's
//! `lib/rubocop/cop/style/array_intersect_with_single_element.rb`.
//!
//! Upstream's `alias on_csend on_send` wires safe-navigation calls through
//! the same handler, but `single_element`'s node-matcher pattern
//! (`(send _ _ $(array $_))`) is literally typed `send`, which never
//! matches a `csend` node in whitequark -- so a safe-navigation call always
//! falls through `return unless array` unmatched. This port checks
//! `is_safe_navigation` directly to the same effect.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use crate::layout::space_inside_array_percent_literal::array_percent_type;

/// RuboCop's `MSG`.
const MSG: &str = "Use `include?(element)` instead of `intersect?([element])`.";

/// Use `include?(element)` instead of `intersect?([element])`.
#[derive(Debug, Clone)]
pub struct ArrayIntersectWithSingleElement;

impl Rule for ArrayIntersectWithSingleElement {
    const META: RuleMeta = RuleMeta {
        name: "Style/ArrayIntersectWithSingleElement",
        department: Department::Style,
        summary: "Use `include?(element)` instead of `intersect?([element])`.",
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
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return; // see the module doc.
        }
        if call.name().as_slice() != b"intersect?" {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };
        let Some(arguments) = call.arguments() else { return };
        let args = arguments.arguments();
        if args.len() != 1 {
            return;
        }
        let Some(array) = args.iter().next().and_then(|a| a.as_array_node()) else { return };
        let elements = array.elements();
        if elements.len() != 1 {
            return;
        }
        let element = elements.iter().next().expect("checked len == 1");
        if element.kind() == NodeKind::SplatNode {
            return; // `[*foo]` is not a single element.
        }

        let offense_span = Span::new(message_loc.span().start, node.span().end);

        let replacement = array
            .opening_loc()
            .and_then(|loc| array_percent_type(ctx.text(loc.span())))
            .map_or_else(|| ctx.text(element.span()).to_vec(), |_| symbol_inspect(&element, ctx));

        let edits = vec![
            Edit::replace(message_loc.span(), b"include?".to_vec()),
            Edit::replace(array.as_node().span(), replacement),
        ];
        ctx.report_with_fix(
            &Self::META,
            offense_span,
            MSG,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// `element.value.inspect` for a percent-literal array's sole element: a
/// symbol's (`%i`) or string's (`%w`) `Symbol#inspect`/`String#inspect`.
fn symbol_inspect(element: &Node<'_>, ctx: &Context<'_>) -> Vec<u8> {
    if let Some(sym) = element.as_symbol_node() {
        let mut out = vec![b':'];
        out.extend_from_slice(sym.unescaped());
        return out;
    }
    if let Some(s) = element.as_string_node() {
        let mut out = vec![b'"'];
        out.extend_from_slice(s.unescaped());
        out.push(b'"');
        return out;
    }
    ctx.text(element.span()).to_vec()
}
