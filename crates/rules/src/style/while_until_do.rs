//! `Style/WhileUntilDo`, ported from RuboCop's
//! `lib/rubocop/cop/style/while_until_do.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Do not use `do` with multi-line `%s`.";

/// Checks for redundant do after while or until.
#[derive(Debug, Clone)]
pub struct WhileUntilDo;

impl Rule for WhileUntilDo {
    const META: RuleMeta = RuleMeta {
        name: "Style/WhileUntilDo",
        department: Department::Style,
        summary: "Checks for redundant do after while or until.",
        explanation: "Checks for uses of `do` in multi-line `while/until` statements.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::WhileNode, NodeKind::UntilNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (predicate, do_keyword_loc, statements, keyword) = match node.kind() {
            NodeKind::WhileNode => {
                let w = node.as_while_node().expect("kind matched");
                (w.predicate(), w.do_keyword_loc(), w.statements(), "while")
            }
            NodeKind::UntilNode => {
                let u = node.as_until_node().expect("kind matched");
                (u.predicate(), u.do_keyword_loc(), u.statements(), "until")
            }
            _ => return,
        };
        let Some(do_keyword_loc) = do_keyword_loc else { return };
        let do_span = do_keyword_loc.span();
        if ctx.is_single_line(node.span()) {
            return;
        }
        if let Some(statements) = &statements {
            if ctx.same_line(do_span, statements.as_node().span()) {
                return;
            }
        }

        let message = MSG.replace("%s", keyword);
        let do_range = Span::new(predicate.span().end, do_span.end);
        ctx.report_with_fix(
            &Self::META,
            do_span,
            message,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(do_range)] },
        );
    }
}
