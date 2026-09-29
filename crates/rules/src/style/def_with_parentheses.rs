//! `Style/DefWithParentheses`, ported from RuboCop's
//! `lib/rubocop/cop/style/def_with_parentheses.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Omit the parentheses in defs when the method doesn't accept any arguments.";

/// `Style/DefWithParentheses`.
#[derive(Debug, Clone)]
pub struct DefWithParentheses;

impl Rule for DefWithParentheses {
    const META: RuleMeta = RuleMeta {
        name: "Style/DefWithParentheses",
        department: Department::Style,
        summary: "Use def with parentheses when there are arguments.",
        explanation: "Checks for parentheses in the definition of a method, \
            that does not take any arguments. Both instance and class/singleton \
            methods are checked.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        if def.parameters().is_some() {
            return;
        }
        let Some(lparen) = def.lparen_loc() else { return };
        let Some(rparen) = def.rparen_loc() else { return };
        let arguments_range = Span::new(lparen.span().start, rparen.span().end);

        if parentheses_required(ctx, &def, arguments_range) {
            return;
        }

        ctx.report_with_fix(
            &Self::META,
            arguments_range,
            MSG,
            linter::Fix {
                applicability: linter::Applicability::Safe,
                edits: vec![linter::Edit::delete(arguments_range)],
            },
        );
    }
}

/// RuboCop's `parentheses_required?`.
fn parentheses_required(
    ctx: &Context<'_>,
    def: &ruby_ast::node::DefNode<'_>,
    arguments_range: Span,
) -> bool {
    let end_pos = arguments_range.end;
    let file_len = u32::try_from(ctx.source().bytes().len()).unwrap_or(u32::MAX);
    let token_after_argument =
        if end_pos < file_len { ctx.text(Span::new(end_pos, end_pos + 1)) } else { &[] };

    // A `;` after the parentheses (e.g. `def foo(); end`) already separates the
    // signature from the body, so the parentheses can be removed (`def foo; end`).
    if token_after_argument == b";" {
        return false;
    }

    let endless = def.equal_loc().is_some();
    if ctx.is_single_line(def.as_node().span()) && !endless {
        return true;
    }

    token_after_argument == b"="
}
