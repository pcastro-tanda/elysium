//! `Style/ColonMethodDefinition`, ported from RuboCop's
//! `lib/rubocop/cop/style/colon_method_definition.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

const MSG: &str = "Do not use `::` for defining class methods.";

/// Do not use :: for defining class methods.
#[derive(Debug, Clone)]
pub struct ColonMethodDefinition;

impl Rule for ColonMethodDefinition {
    const META: RuleMeta = RuleMeta {
        name: "Style/ColonMethodDefinition",
        department: Department::Style,
        summary: "Do not use :: for defining class methods.",
        explanation: "Checks for class methods that are defined using the `::` \
            operator instead of the `.` operator.",
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
        let Some(operator_loc) = def.operator_loc() else { return };
        let span = operator_loc.span();
        if ctx.text(span) != b"::" {
            return;
        }
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, b".".to_vec())],
            },
        );
    }
}
