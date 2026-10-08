//! `Style/MultilineInPatternThen`, ported from RuboCop's
//! `lib/rubocop/cop/style/multiline_in_pattern_then.rb`.
//!
//! `require_then?` upstream compares `in_pattern_node` (the whole `InNode`,
//! which starts at its own `in` keyword) against its `body` (the `in`
//! clause's `StatementsNode`): `then` is required only when the pattern is
//! itself single-line *and* the body starts on that same source line.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::InNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Side;

/// RuboCop's `MSG`.
const MSG: &str = "Do not use `then` for multiline `in` statement.";

/// Do not use `then` for multi-line `in` statement.
#[derive(Debug, Clone)]
pub struct MultilineInPatternThen {
    /// `minimum_target_ruby_version 2.7`.
    enabled: bool,
}

impl Rule for MultilineInPatternThen {
    const META: RuleMeta = RuleMeta {
        name: "Style/MultilineInPatternThen",
        department: Department::Style,
        summary: "Do not use `then` for multi-line `in` statement.",
        explanation: "Checks uses of the `then` keyword in multi-line `in` statement.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::InNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { enabled: options.target_ruby_version() >= 2.7 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled {
            return;
        }
        let in_node = node.as_in_node().expect("kind matched");
        let Some(then_loc) = in_node.then_loc() else { return };
        if require_then(&in_node, node, ctx) {
            return;
        }

        let span = then_loc.span();
        let remove = ctx.with_surrounding_space(span, Side::Left, false, false);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(remove)] },
        );
    }
}

/// `require_then?`: `then` is required for a write `in` pattern when its
/// body sits on the same line as the `in` keyword, which can only happen if
/// the pattern itself is single-line.
fn require_then(in_node: &InNode<'_>, node: &Node<'_>, ctx: &Context<'_>) -> bool {
    if !ctx.is_single_line(in_node.pattern().span()) {
        return true;
    }
    let Some(body) = in_node.statements() else { return false };
    ctx.same_line(node.span(), body.as_node().span())
}
