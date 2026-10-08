//! `Style/SuperWithArgsParentheses`, ported from RuboCop's
//! `lib/rubocop/cop/style/super_with_args_parentheses.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use parentheses for `super` with arguments.";

/// Use parentheses for `super` with arguments.
#[derive(Debug, Clone)]
pub struct SuperWithArgsParentheses;

impl Rule for SuperWithArgsParentheses {
    const META: RuleMeta = RuleMeta {
        name: "Style/SuperWithArgsParentheses",
        department: Department::Style,
        summary: "Use parentheses for `super` with arguments.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::SuperNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(sup) = node.as_super_node() else { return };
        if sup.lparen_loc().is_some() {
            return;
        }
        let Some(arguments) = sup.arguments() else { return };
        let args = arguments.arguments();
        let (Some(first), Some(last)) = (args.iter().next(), args.iter().last()) else {
            return;
        };
        let keyword_end = sup.keyword_loc().span().end;
        let paren_span = Span::new(keyword_end, first.span().start);
        let edits = vec![
            Edit::replace(paren_span, b"(".to_vec()),
            Edit::insert(last.span().end, b")".to_vec()),
        ];
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            MSG,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}
