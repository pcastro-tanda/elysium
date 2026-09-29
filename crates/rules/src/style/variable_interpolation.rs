//! `Style/VariableInterpolation`, ported from RuboCop's
//! `lib/rubocop/cop/style/variable_interpolation.rb`.
//!
//! Whitequark's `dstr`/`dsym`/`xstr`/`regexp` children embed a shorthand
//! `#@var`/`#$var`/`#$1` variable/reference node directly, while Prism
//! always wraps that shorthand in a distinct
//! [`NodeKind::EmbeddedVariableNode`] (`operator_loc` is the `#`,
//! `variable` is the actual read/reference node). Since that shorthand
//! syntax can only ever hold a global/instance/class variable read or a
//! back-/nth-reference (RuboCop-AST's `variable?`/`reference?`), every
//! `EmbeddedVariableNode` is unconditionally an offense -- no extra
//! `variable?`/`reference?` filtering is needed. The reported range and the
//! autocorrect both target the inner `variable` node only, matching
//! upstream's `add_offense(var_node)` / `corrector.replace(var_node, ...)`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Replace interpolated variable `%<variable>s` with expression `#{%<variable>s}`.";

/// Checks for variable interpolation (like `"#@ivar"`).
#[derive(Debug, Clone)]
pub struct VariableInterpolation;

impl Rule for VariableInterpolation {
    const META: RuleMeta = RuleMeta {
        name: "Style/VariableInterpolation",
        department: Department::Style,
        summary: "Don't interpolate global, instance and class variables directly in strings.",
        explanation: "Checks for variable interpolation (like \"#@ivar\").",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::EmbeddedVariableNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(embedded) = node.as_embedded_variable_node() else { return };
        let variable = embedded.variable();
        let span = variable.span();
        let source = String::from_utf8_lossy(ctx.text(span)).into_owned();
        let message = MSG.replace("%<variable>s", &source);
        let replacement = format!("{{{source}}}");
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement.into_bytes())],
            },
        );
    }
}
