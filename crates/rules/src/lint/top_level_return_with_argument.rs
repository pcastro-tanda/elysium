//! `Lint/TopLevelReturnWithArgument`, ported from RuboCop's
//! `lib/rubocop/cop/lint/top_level_return_with_argument.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Top level return with argument detected.";

/// Detects top level return statements with argument.
#[derive(Debug, Clone)]
pub struct TopLevelReturnWithArgument;

impl Rule for TopLevelReturnWithArgument {
    const META: RuleMeta = RuleMeta {
        name: "Lint/TopLevelReturnWithArgument",
        department: Department::Lint,
        summary: "Checks for top level return with arguments.",
        explanation: "If there is a top-level return statement with an \
            argument, then the argument is always ignored. This is detected \
            automatically since Ruby 2.7.\n\n\
            ```ruby\n\
            # bad\n\
            return 1\n\n\
            # good\n\
            return\n\
            ```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ReturnNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(return_node) = node.as_return_node() else {
            return;
        };
        if return_node.arguments().is_none() {
            return;
        }
        let is_top_level = ctx.ancestors().iter().all(|a| {
            !matches!(a.kind, NodeKind::BlockNode | NodeKind::LambdaNode | NodeKind::DefNode)
        });
        if !is_top_level {
            return;
        }
        let span = node.span();
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, b"return".to_vec())],
            },
        );
    }
}
