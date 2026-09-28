//! `Lint/BigDecimalNew`, ported from RuboCop's
//! `lib/rubocop/cop/lint/big_decimal_new.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "`BigDecimal.new()` is deprecated. Use `BigDecimal()` instead.";

/// `BigDecimal.new()` is deprecated. Use `BigDecimal()` instead.
#[derive(Debug, Clone)]
pub struct BigDecimalNew;

impl Rule for BigDecimalNew {
    const META: RuleMeta = RuleMeta {
        name: "Lint/BigDecimalNew",
        department: Department::Lint,
        summary: "`BigDecimal.new()` is deprecated. Use `BigDecimal()` instead.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"new" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };

        let (const_name, cbase_span) = if let Some(c) = receiver.as_constant_read_node() {
            (c.name().as_slice(), None)
        } else if let Some(path) = receiver.as_constant_path_node() {
            if path.parent().is_some() {
                return;
            }
            let Some(name) = path.name() else { return };
            (name.as_slice(), Some(path.delimiter_loc().span()))
        } else {
            return;
        };

        if const_name != b"BigDecimal" {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };
        let selector_span = message_loc.span();

        let mut edits = vec![Edit::delete(selector_span)];
        if let Some(operator_loc) = call.call_operator_loc() {
            edits.push(Edit::delete(operator_loc.span()));
        }
        if let Some(cbase_span) = cbase_span {
            edits.push(Edit::delete(cbase_span));
        }

        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, selector_span, MSG, fix);
    }
}
