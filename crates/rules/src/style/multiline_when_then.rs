//! `Style/MultilineWhenThen`, ported from RuboCop's
//! `lib/rubocop/cop/style/multiline_when_then.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::WhenNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// RuboCop's `MSG`.
const MSG: &str = "Do not use `then` for multiline `when` statement.";

/// `Style/MultilineWhenThen`: flags a `then` keyword on a `when` branch
/// whose body doesn't start on the same line as `when`.
#[derive(Debug, Clone)]
pub struct MultilineWhenThen;

impl Rule for MultilineWhenThen {
    const META: RuleMeta = RuleMeta {
        name: "Style/MultilineWhenThen",
        department: Department::Style,
        summary: "Do not use then for multi-line when statement.",
        explanation: "Checks uses of the `then` keyword in multi-line `when` statements.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::WhenNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(when) = node.as_when_node() else { return };
        let Some(then_loc) = when.then_keyword_loc() else { return };
        if require_then(&when, ctx) {
            return;
        }

        let span = then_loc.span();
        let removed = ctx.with_surrounding_space(span, Side::Left, false, false);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(removed)] },
        );
    }
}

/// RuboCop's `require_then?`: `then` is required (so no offense is
/// registered) when the `when` conditions themselves span multiple lines,
/// or when the branch's body starts on the same line as `when`.
fn require_then(when: &WhenNode<'_>, ctx: &Context<'_>) -> bool {
    let conditions = when.conditions();
    if let (Some(first), Some(last)) = (conditions.first(), conditions.last()) {
        let span = Span::new(first.span().start, last.span().end);
        if !ctx.is_single_line(span) {
            return true;
        }
    }

    let Some(statements) = when.statements() else { return false };
    ctx.same_line(when.keyword_loc().span(), statements.location().span())
}
