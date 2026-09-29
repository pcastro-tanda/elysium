//! `Style/EndBlock`, ported from RuboCop's
//! `lib/rubocop/cop/style/end_block.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

const MSG: &str = "Avoid the use of `END` blocks. Use `Kernel#at_exit` instead.";

/// Avoid the use of END blocks.
#[derive(Debug, Clone)]
pub struct EndBlock;

impl Rule for EndBlock {
    const META: RuleMeta = RuleMeta {
        name: "Style/EndBlock",
        department: Department::Style,
        summary: "Avoid the use of END blocks.",
        explanation: "Checks for `END` blocks. `END` blocks are Perl-style constructs \
            and `Kernel#at_exit` is the idiomatic Ruby alternative, as it's \
            explicit and can be used anywhere.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::PostExecutionNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(postexe) = node.as_post_execution_node() else { return };
        let span = postexe.keyword_loc().span();
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, b"at_exit".to_vec())],
            },
        );
    }
}
