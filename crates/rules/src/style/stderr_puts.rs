//! `Style/StderrPuts`, ported from RuboCop's
//! `lib/rubocop/cop/style/stderr_puts.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `warn` instead of `%<bad>s` to allow such output to be disabled.";

/// Use `warn` instead of `$stderr.puts`.
#[derive(Debug, Clone)]
pub struct StderrPuts;

impl Rule for StderrPuts {
    const META: RuleMeta = RuleMeta {
        name: "Style/StderrPuts",
        department: Department::Style,
        summary: "Use `warn` instead of `$stderr.puts`.",
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
        if call.is_safe_navigation() {
            return;
        }
        if call.name().as_slice() != b"puts" {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        if arguments.arguments().is_empty() {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !is_stderr_receiver(&receiver) {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };

        let range = Span::new(node.span().start, message_loc.span().end);
        let receiver_source = String::from_utf8_lossy(ctx.text(receiver.span()));
        let message = MSG.replace("%<bad>s", &format!("{receiver_source}.puts"));

        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, b"warn".to_vec())],
            },
        );
    }
}

fn is_stderr_receiver(receiver: &Node<'_>) -> bool {
    if let Some(gvar) = receiver.as_global_variable_read_node() {
        return gvar.name().as_slice() == b"$stderr";
    }
    if let Some(const_read) = receiver.as_constant_read_node() {
        return const_read.name().as_slice() == b"STDERR";
    }
    if let Some(const_path) = receiver.as_constant_path_node() {
        return const_path.parent().is_none()
            && const_path.name().is_some_and(|name| name.as_slice() == b"STDERR");
    }
    false
}
