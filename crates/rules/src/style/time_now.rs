//! `Style/TimeNow`, ported from RuboCop's
//! `lib/rubocop/cop/style/time_now.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
/// RuboCop's `MSG`.
const MSG: &str = "Prefer `Time.now` over `Time.new` to retrieve the current time.";

/// Prefer `Time.now` over `Time.new` when retrieving the current system time.
#[derive(Debug, Clone)]
pub struct TimeNow;

impl Rule for TimeNow {
    const META: RuleMeta = RuleMeta {
        name: "Style/TimeNow",
        department: Department::Style,
        summary: "Prefer `Time.now` over `Time.new` when retrieving the current system time.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
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
        if call.arguments().is_some() {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !is_bare_or_toplevel_const(&receiver) {
            return;
        }
        if const_name(&receiver).as_deref() != Some("Time") {
            return;
        }

        let Some(message_loc) = call.message_loc() else { return };
        let span = node.span();

        let replace_span = Span::new(message_loc.span().start, span.end);
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(replace_span, b"now".to_vec())],
        };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}
