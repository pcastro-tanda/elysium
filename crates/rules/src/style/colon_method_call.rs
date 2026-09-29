//! `Style/ColonMethodCall`, ported from RuboCop's
//! `lib/rubocop/cop/style/colon_method_call.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeKind};

const MSG: &str = "Do not use `::` for method calls.";

/// Do not use :: for method call.
#[derive(Debug, Clone)]
pub struct ColonMethodCall;

impl Rule for ColonMethodCall {
    const META: RuleMeta = RuleMeta {
        name: "Style/ColonMethodCall",
        department: Department::Style,
        summary: "Do not use :: for method call.",
        explanation: "Checks for methods invoked via the `::` operator instead \
            of the `.` operator (like `FileUtils::rmdir` instead of \
            `FileUtils.rmdir`). The `::` operator is conventionally used to \
            reference constants, so using it for method calls can be misleading.",
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
        if call.receiver().is_none() {
            return;
        }
        let Some(op_loc) = call.call_operator_loc() else { return };
        if ctx.text(op_loc.span()) != b"::" {
            return;
        }
        if camel_case_method(&call) {
            return;
        }
        if is_java_interop(&call) {
            return;
        }

        let span = op_loc.span();
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

fn camel_case_method(call: &CallNode<'_>) -> bool {
    call.name().as_slice().first().is_some_and(u8::is_ascii_uppercase)
}

/// Ignore Java interop code like `Java::int` or `Java::com::method`.
fn is_java_interop(call: &CallNode<'_>) -> bool {
    let mut receiver = call.receiver();
    loop {
        let Some(current) = receiver else { return false };
        if let Some(inner_call) = current.as_call_node() {
            if let Some(inner_receiver) = inner_call.receiver() {
                receiver = Some(inner_receiver);
                continue;
            }
        }
        return is_java_root(&current);
    }
}

fn is_java_root(node: &Node<'_>) -> bool {
    node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"Java")
}
