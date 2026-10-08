//! `Style/CollectionQuerying`, ported from RuboCop's
//! `lib/rubocop/cop/style/collection_querying.rb`.
//!
//! # Matched shape
//!
//! Upstream's `count_predicate` pattern has two alternatives for the
//! receiver -- a plain `.count` call, or an `any_block`-wrapped one (`.count
//! { ... }`/numblock/itblock) -- because whitequark represents a call with
//! a literal block as a separate `block`/`numblock`/`itblock` node wrapping
//! the call. Prism instead keeps the call as a plain
//! [`NodeKind::CallNode`] whose own span already extends through an
//! attached block (reached via `CallNode::block`, same as `Style/DataInheritance`'s
//! superclass handling), so both upstream alternatives collapse into one
//! check here: a `.count` call with an explicit receiver and either no
//! arguments or a single block-pass argument (a literal block, if any,
//! needs no special handling since it is simply left untouched by every
//! edit below).
//!
//! The *outer* comparison call must be a plain `send` (RuboCop's pattern's
//! outermost node type), excluding safe navigation on the final comparison
//! (`x.count&.positive?` does not register) while still allowing it on the
//! inner `.count` call (`x&.count(&:foo?).positive?` does, via the `call`
//! type used there) -- `is_safe_navigation` on the outer call alone
//! reproduces this asymmetry.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// RuboCop's `MSG`.
const MSG_TEMPLATE: &str = "Use `%<prefer>s` instead.";

/// Prefer `Enumerable` predicate methods over expressions with `count`.
#[derive(Debug, Clone)]
pub struct CollectionQuerying {
    /// `AllCops/ActiveSupportExtensionsEnabled`.
    active_support_extensions_enabled: bool,
}

impl Rule for CollectionQuerying {
    const META: RuleMeta = RuleMeta {
        name: "Style/CollectionQuerying",
        department: Department::Style,
        summary: "Prefer `Enumerable` predicate methods over expressions with `count`.",
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
        let active_support_extensions_enabled = options
            .peer("AllCops", "ActiveSupportExtensionsEnabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(false);
        Ok(Self { active_support_extensions_enabled })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(outer) = node.as_call_node() else { return };
        if outer.is_safe_navigation() {
            return;
        }
        let method = outer.name().as_slice();
        if !matches!(method, b"positive?" | b">" | b"!=" | b"zero?" | b"==") {
            return;
        }
        let Some(receiver) = outer.receiver() else { return };
        let Some(count_call) = receiver.as_call_node() else { return };
        if !is_count_call(&count_call) {
            return;
        }
        let Some(count_selector) = count_call.message_loc() else { return };

        let first_arg_value = first_int_arg(&outer);
        let Some(replacement) = replacement_method(method, first_arg_value) else { return };
        if replacement == "many?" && !self.active_support_extensions_enabled {
            return;
        }

        let offense_range = Span::new(count_selector.span().start, node.span().end);
        let message = MSG_TEMPLATE.replace("%<prefer>s", replacement);

        let removal_start = outer
            .call_operator_loc()
            .or_else(|| outer.message_loc())
            .map_or(node.span().end, |l| l.span().start);
        let removal_range = ctx.with_surrounding_space(
            Span::new(removal_start, node.span().end),
            Side::Left,
            true,
            false,
        );

        let edits = vec![
            Edit::replace(count_selector.span(), replacement.as_bytes().to_vec()),
            Edit::delete(removal_range),
        ];
        ctx.report_with_fix(
            &Self::META,
            offense_range,
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// `(call !nil? :count (block-pass _)?)`: a `.count` call with an explicit
/// receiver and either no arguments or a single block-pass argument.
fn is_count_call(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() != b"count" || call.receiver().is_none() {
        return false;
    }
    let args = call.arguments().map_or(Vec::new(), |a| a.arguments().iter().collect::<Vec<_>>());
    args.is_empty() || (args.len() == 1 && args[0].kind() == NodeKind::BlockArgumentNode)
}

/// The outer comparison call's sole integer-literal argument, if any.
fn first_int_arg(call: &CallNode<'_>) -> Option<i64> {
    let args = call.arguments()?.arguments();
    if args.len() != 1 {
        return None;
    }
    let value: i32 = args.iter().next()?.as_integer_node()?.value().try_into().ok()?;
    Some(i64::from(value))
}

/// RuboCop's `REPLACEMENTS`.
fn replacement_method(method: &[u8], arg: Option<i64>) -> Option<&'static str> {
    match (method, arg) {
        (b"positive?", None) | (b">" | b"!=", Some(0)) => Some("any?"),
        (b"zero?", None) | (b"==", Some(0)) => Some("none?"),
        (b"==", Some(1)) => Some("one?"),
        (b">", Some(1)) => Some("many?"),
        _ => None,
    }
}
