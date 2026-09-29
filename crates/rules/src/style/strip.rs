//! `Style/Strip`, ported from RuboCop's
//! `lib/rubocop/cop/style/strip.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `strip` instead of `%<methods>s`.";

/// Use `strip` instead of `lstrip.rstrip`.
#[derive(Debug, Clone)]
pub struct Strip;

impl Rule for Strip {
    const META: RuleMeta = RuleMeta {
        name: "Style/Strip",
        department: Department::Style,
        summary: "Use `strip` instead of `lstrip.rstrip`.",
        explanation: "",
        enabled_by_default: true,
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
        let outer_name = call.name().as_slice();
        if outer_name != b"lstrip" && outer_name != b"rstrip" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(inner_call) = receiver.as_call_node() else { return };
        let inner_name = inner_call.name().as_slice();
        let matched = (outer_name == b"lstrip" && inner_name == b"rstrip")
            || (outer_name == b"rstrip" && inner_name == b"lstrip");
        if !matched {
            return;
        }
        let Some(first_send_selector) = inner_call.message_loc() else { return };
        let start = first_send_selector.span().start;
        let end = node.span().end;
        let range = ruby_source::Span::new(start, end);
        let methods = String::from_utf8_lossy(ctx.text(range)).into_owned();
        let message = MSG.replace("%<methods>s", &methods);
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, b"strip".to_vec())],
            },
        );
    }
}
