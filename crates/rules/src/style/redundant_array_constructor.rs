//! `Style/RedundantArrayConstructor`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_array_constructor.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{ext, LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MSG: &str = "Remove the redundant `Array` constructor.";

/// Checks for the instantiation of array using redundant `Array` constructor.
#[derive(Debug, Clone)]
pub struct RedundantArrayConstructor;

impl Rule for RedundantArrayConstructor {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantArrayConstructor",
        department: Department::Style,
        summary: "Checks for the instantiation of array using redundant `Array` constructor.",
        explanation: "Checks for the instantiation of an array using a redundant `Array` \
            constructor. Autocorrect replaces it with an array literal which is the simplest \
            and fastest.",
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
        if call.is_safe_navigation() {
            return;
        }
        let node_span = node.location().span();
        let name = call.name().as_slice();

        if name == b"new" {
            let Some(receiver) = call.receiver() else { return };
            if !is_array_const(&receiver) {
                return;
            }
            let Some(array_literal) = sole_array_argument(&call) else { return };
            let Some(message_loc) = call.message_loc() else { return };
            let range = Span::new(receiver.location().span().start, message_loc.span().end);
            let replacement = ctx.text(array_literal.location().span()).to_vec();
            report(ctx, range, node_span, replacement);
        } else if name == b"[]" {
            let Some(receiver) = call.receiver() else { return };
            if !is_array_const(&receiver) {
                return;
            }
            let range = receiver.location().span();
            let replacement = ctx.text(Span::new(range.end, node_span.end)).to_vec();
            report(ctx, range, node_span, replacement);
        } else if name == b"Array" {
            if call.receiver().is_some() {
                return;
            }
            let Some(array_literal) = sole_array_argument(&call) else { return };
            let Some(message_loc) = call.message_loc() else { return };
            let range = message_loc.span();
            let replacement = ctx.text(array_literal.location().span()).to_vec();
            report(ctx, range, node_span, replacement);
        }
    }
}

/// RuboCop's `(const {nil? cbase} :Array)`.
fn is_array_const(node: &Node<'_>) -> bool {
    ext::is_bare_or_toplevel_const(node) && ext::const_name(node).as_deref() == Some("Array")
}

/// The call's sole argument, when it is an array literal (`$(array ...)`).
fn sole_array_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let args = call.arguments()?;
    let list = args.arguments();
    if list.len() != 1 {
        return None;
    }
    let arg = list.first().expect("checked len");
    arg.as_array_node()?;
    Some(arg)
}

fn report(ctx: &mut Context<'_>, range: Span, node_span: Span, replacement: Vec<u8>) {
    ctx.report_with_fix(
        &RedundantArrayConstructor::META,
        range,
        MSG,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node_span, replacement)],
        },
    );
}
