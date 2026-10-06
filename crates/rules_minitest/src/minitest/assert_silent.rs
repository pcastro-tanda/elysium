//! `Minitest/AssertSilent`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_silent.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{Node, NodeKind};

const MSG: &str = "Prefer using `assert_silent`.";

/// Enforces the test to use `assert_silent { ... }` instead of using
/// `assert_output('', '') { ... }`.
#[derive(Debug, Clone)]
pub struct AssertSilent;

impl Rule for AssertSilent {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertSilent",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `assert_silent { ... }` instead of using \
                  `assert_output('', '') { ... }`.",
        explanation: "Enforces the test to use `assert_silent { ... }` instead of using \
                      `assert_output('', '') { ... }`.\n\n```ruby\n# bad\n\
                      assert_output('', '') { puts object.do_something }\n\n# good\n\
                      assert_silent { puts object.do_something }\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // (block (send nil? :assert_output #empty_string? #empty_string?) ...)
        // `on_block` only: not a `numblock`/`itblock`, and `&.` is a `csend`.
        if call.receiver().is_some()
            || call.is_safe_navigation()
            || call.name().as_slice() != b"assert_output"
        {
            return;
        }
        let Some(block) = call.block().and_then(|block| block.as_block_node()) else { return };
        if block.parameters().is_some_and(|p| {
            p.as_numbered_parameters_node().is_some() || p.as_it_parameters_node().is_some()
        }) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let arguments = arguments.arguments();
        if arguments.len() != 2 || !arguments.iter().all(|arg| is_empty_string(&arg)) {
            return;
        }

        let span = call_span_excluding_block(&call);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, b"assert_silent".to_vec())],
            },
        );
    }
}

/// `node.str_type? && node.value.empty?`
fn is_empty_string(node: &Node<'_>) -> bool {
    node.as_string_node().is_some_and(|string| string.unescaped().is_empty())
}
