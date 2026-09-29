//! `Style/ArrayJoin`, ported from RuboCop's
//! `lib/rubocop/cop/style/array_join.rb`.
//!
//! Upstream's matcher `(send $array :* $str)` only requires the method to be
//! `*` with a receiver and a single plain string-literal argument (a Prism
//! `StringNode`, i.e. not an interpolated string) -- it does not actually
//! check that the receiver is an array literal, despite what the class doc
//! comment implies.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "Favor `Array#join` over `Array#*`.";

/// Use Array#join instead of Array#*.
#[derive(Debug, Clone)]
pub struct ArrayJoin;

impl Rule for ArrayJoin {
    const META: RuleMeta = RuleMeta {
        name: "Style/ArrayJoin",
        department: Department::Style,
        summary: "Use Array#join instead of Array#*.",
        explanation: "Checks for uses of `*` as a substitute for `Array#join`. \
            Using `join` is clearer about intent and more readable than \
            overloading the `*` operator for string conversion.\n\n\
            Not all cases can be reliably checked, due to Ruby's dynamic \
            types, so we consider only cases when the first argument is an \
            array literal or the second is a string literal.",
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
        if call.name().as_slice() != b"*" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(arguments) = call.arguments() else { return };
        let args = arguments.arguments();
        if args.len() != 1 {
            return;
        }
        let Some(join_arg) = args.iter().next().and_then(|a| a.as_string_node()) else {
            return;
        };
        let Some(message_loc) = call.message_loc() else { return };

        let receiver_src = ctx.text(receiver.span()).to_vec();
        let arg_src = ctx.text(join_arg.location().span()).to_vec();
        let mut replacement = receiver_src;
        replacement.extend_from_slice(b".join(");
        replacement.extend_from_slice(&arg_src);
        replacement.push(b')');

        ctx.report_with_fix(
            &Self::META,
            message_loc.span(),
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(node.span(), replacement)],
            },
        );
    }
}
