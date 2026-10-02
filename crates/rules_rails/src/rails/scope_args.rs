//! `Rails/ScopeArgs`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/scope_args.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `lambda`/`proc` instead of a plain method call.";

/// Checks the arguments of `ActiveRecord` scopes.
#[derive(Debug, Clone)]
pub struct ScopeArgs;

impl Rule for ScopeArgs {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ScopeArgs",
        department: Department::Rails,
        summary: "Checks the arguments of ActiveRecord scopes.",
        explanation: "Checks for scope calls where it was passed a method (usually a scope) \
                      instead of a lambda/proc.\n\n```ruby\n# bad\nscope :something, \
                      where(something: true)\n\n# good\nscope :something, -> { \
                      where(something: true) }\n```",
        enabled_by_default: true,
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
        // `(send nil? :scope _ $send)`.
        let Some(call) = node.as_call_node() else { return };
        if call.receiver().is_some()
            || call.is_safe_navigation()
            || call.name().as_slice() != b"scope"
        {
            return;
        }
        // A `&block` argument is a further `send` argument in whitequark.
        if call.block().is_some_and(|block| block.as_block_argument_node().is_some()) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut arguments = arguments.arguments().iter();
        let (Some(_), Some(second), None) = (arguments.next(), arguments.next(), arguments.next())
        else {
            return;
        };
        // A call with a literal block is a `block` node, not a `send`.
        let Some(second_call) = second.as_call_node() else { return };
        if second_call.is_safe_navigation() || second_call.block().is_some() {
            return;
        }
        let span = second.span();
        let replacement = format!("-> {{ {} }}", String::from_utf8_lossy(ctx.text(span)));
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement.into_bytes())],
            },
        );
    }
}
