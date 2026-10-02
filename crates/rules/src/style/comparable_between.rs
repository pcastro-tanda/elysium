//! `Style/ComparableBetween`, ported from RuboCop's
//! `lib/rubocop/cop/style/comparable_between.rb`.
//!
//! # Matched shape
//!
//! Upstream matches via two node-matcher alternatives
//! (`logical_comparison_between_by_min_first?`/`..._by_max_first?`) whose
//! capture order depends on which side of each `>=`/`<=` happened to carry
//! the shared operand, and -- since an `and` of two flexible `>=`/`<=`
//! comparisons can satisfy *both* alternatives for the same node -- relies
//! on RuboCop's offense deduplication (two `add_offense(node)` calls at the
//! same range keep only the correction that registers last) to land on the
//! right grouping. Rather than reproduce that pattern-order/dedup dance,
//! [`term_role`] derives each side's role directly from comparison
//! semantics: a shared `value` is found by text match between the two
//! calls' operands, then each call's *other* operand is classified as the
//! `min` or `max` bound from the direction of its own operator and which
//! side of it carried `value` (`value >= min`, `min <= value`, `value <=
//! max`, `max >= value`). The `and` only qualifies when one side yields a
//! `min` and the other a `max`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG_TEMPLATE: &str = "Prefer `%<prefer>s` over logical comparison.";

/// Enforces the use of `Comparable#between?` instead of logical comparison.
#[derive(Debug, Clone)]
pub struct ComparableBetween;

impl Rule for ComparableBetween {
    const META: RuleMeta = RuleMeta {
        name: "Style/ComparableBetween",
        department: Department::Style,
        summary: "Enforces the use of `Comparable#between?` instead of logical comparison.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::AndNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(and) = node.as_and_node() else { return };
        let Some(left) = and.left().as_call_node() else { return };
        let Some(right) = and.right().as_call_node() else { return };

        let Some((la, lb, l_ge)) = comparison_operands(&left) else { return };
        let Some((ra, rb, r_ge)) = comparison_operands(&right) else { return };

        let src = |n: &Node<'_>| ctx.text(n.span());
        let value = if src(&la) == src(&ra) || src(&la) == src(&rb) {
            la
        } else if src(&lb) == src(&ra) || src(&lb) == src(&rb) {
            lb
        } else {
            return;
        };
        let value_src = src(&value);

        let Some((l_is_min, l_bound)) = term_role(&la, &lb, l_ge, value_src, ctx) else { return };
        let Some((r_is_min, r_bound)) = term_role(&ra, &rb, r_ge, value_src, ctx) else { return };
        if l_is_min == r_is_min {
            return;
        }
        let (min, max) = if l_is_min { (l_bound, r_bound) } else { (r_bound, l_bound) };

        let prefer = format!(
            "{}.between?({}, {})",
            String::from_utf8_lossy(src(&value)),
            String::from_utf8_lossy(src(&min)),
            String::from_utf8_lossy(src(&max)),
        );
        let message = MSG_TEMPLATE.replace("%<prefer>s", &prefer);
        let edits = vec![Edit::replace(node.span(), prefer.into_bytes())];
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// `(receiver, argument, is_ge)` for a bare `>=`/`<=` call with exactly one
/// argument; `None` for any other operator or arity.
fn comparison_operands<'pr>(call: &CallNode<'pr>) -> Option<(Node<'pr>, Node<'pr>, bool)> {
    let is_ge = match call.name().as_slice() {
        b">=" => true,
        b"<=" => false,
        _ => return None,
    };
    let receiver = call.receiver()?;
    let args = call.arguments()?.arguments();
    if args.len() != 1 {
        return None;
    }
    Some((receiver, args.iter().next()?, is_ge))
}

/// Classifies one `a op b` comparison (`is_ge`: `op` is `>=`) as
/// contributing a `min` or `max` bound, given which side (`a` or `b`)
/// carries `value` -- `None` when neither side matches `value`.
/// `true` in the returned pair means `min`.
fn term_role<'pr>(
    a: &Node<'pr>,
    b: &Node<'pr>,
    is_ge: bool,
    value_src: &[u8],
    ctx: &Context<'_>,
) -> Option<(bool, Node<'pr>)> {
    if ctx.text(a.span()) == value_src {
        // `value op b`: `value >= b` means `b` is the min; `value <= b`
        // means `b` is the max.
        Some((is_ge, *b))
    } else if ctx.text(b.span()) == value_src {
        // `a op value`: `a >= value` means `value <= a`, so `a` is the max;
        // `a <= value` means `value >= a`, so `a` is the min.
        Some((!is_ge, *a))
    } else {
        None
    }
}
