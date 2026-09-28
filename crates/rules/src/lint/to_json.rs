//! `Lint/ToJSON`, ported from RuboCop's
//! `lib/rubocop/cop/lint/to_json.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "`#to_json` requires an optional argument to be parsable via JSON.generate(obj).";

/// Ensure `#to_json` includes an optional argument.
#[derive(Debug, Clone)]
pub struct ToJSON;

impl Rule for ToJSON {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ToJSON",
        department: Department::Lint,
        summary: "Ensure #to_json includes an optional argument.",
        explanation: "Checks to make sure `#to_json` includes an optional argument. \
            When overriding `#to_json`, callers may invoke JSON generation via \
            `JSON.generate(your_obj)`. Since `JSON#generate` allows for an optional \
            argument, your method should too.",
        enabled_by_default: true,
        severity: Severity::Warning,
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
        if def.name().as_slice() != b"to_json" {
            return;
        }
        let has_arguments = def.parameters().is_some_and(|params| {
            !params.requireds().is_empty()
                || !params.optionals().is_empty()
                || params.rest().is_some()
                || !params.posts().is_empty()
                || !params.keywords().is_empty()
                || params.keyword_rest().is_some()
                || params.block().is_some()
        });
        if has_arguments {
            return;
        }

        let insert_at = def.name_loc().span().end;
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::insert(insert_at, b"(*_args)".to_vec())],
            },
        );
    }
}
