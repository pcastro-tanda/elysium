//! `Style/UnpackFirst`, ported from RuboCop's
//! `lib/rubocop/cop/style/unpack_first.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks for accessing the first element of `String#unpack` instead of using `unpack1`.
#[derive(Debug, Clone)]
pub struct UnpackFirst {
    target_ruby_version: f32,
}

impl Rule for UnpackFirst {
    const META: RuleMeta = RuleMeta {
        name: "Style/UnpackFirst",
        department: Department::Style,
        summary:
            "Checks for accessing the first element of `String#unpack` instead of using `unpack1`.",
        explanation: "Checks for accessing the first element of `String#unpack`\nwhich can be replaced with the shorter method `unpack1`.\n\n# Examples\n\n```ruby\n# bad\n'foo'.unpack('h*').first\n'foo'.unpack('h*')[0]\n'foo'.unpack('h*').slice(0)\n'foo'.unpack('h*').at(0)\n\n# good\n'foo'.unpack1('h*')\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.target_ruby_version < 2.4 {
            return;
        }
        let call = node.as_call_node().expect("kind matched");
        if !matches_first_element_access(&call) {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(unpack_call) = receiver.as_call_node() else { return };
        if unpack_call.name().as_slice() != b"unpack" {
            return;
        }
        let Some(unpack_args) = unpack_call.arguments() else { return };
        let args: Vec<Node<'_>> = unpack_args.arguments().iter().collect();
        let [unpack_arg] = args.as_slice() else { return };
        let Some(selector) = unpack_call.message_loc() else { return };

        let offense_span = Span::new(selector.span().start, node.span().end);
        let format = String::from_utf8_lossy(ctx.text(unpack_arg.span()));
        let current = String::from_utf8_lossy(ctx.text(offense_span));
        let message = format!("Use `unpack1({format})` instead of `{current}`.");

        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![
                Edit::replace(selector.span(), b"unpack1".to_vec()),
                Edit::delete(Span::new(unpack_call.location().span().end, node.span().end)),
            ],
        };
        ctx.report_with_fix(&Self::META, offense_span, message, fix);
    }
}

/// RuboCop's `unpack_and_first_element?` node matcher, restricted to the
/// outer call's own shape (receiver checked separately).
fn matches_first_element_access(call: &CallNode<'_>) -> bool {
    match call.name().as_slice() {
        b"first" => call.arguments().is_none(),
        b"[]" | b"slice" | b"at" => {
            let Some(args) = call.arguments() else { return false };
            let items: Vec<Node<'_>> = args.arguments().iter().collect();
            let [arg] = items.as_slice() else { return false };
            arg.as_integer_node().is_some_and(|n| TryInto::<i32>::try_into(n.value()) == Ok(0))
        }
        _ => false,
    }
}
