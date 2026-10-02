//! `Style/ReverseFind`, ported from RuboCop's
//! `lib/rubocop/cop/style/reverse_find.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Use `rfind` instead.";

/// Use `array.rfind` instead of `array.reverse.find`.
#[derive(Debug, Clone)]
pub struct ReverseFind {
    /// RuboCop's `minimum_target_ruby_version 4.0`.
    enabled: bool,
}

impl Rule for ReverseFind {
    const META: RuleMeta = RuleMeta {
        name: "Style/ReverseFind",
        department: Department::Style,
        summary: "Use `array.rfind` instead of `array.reverse.find`.",
        explanation: "This cop is unsafe because it cannot be guaranteed that the receiver is \
            an `Array` or responds to the replacement method.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { enabled: options.target_ruby_version() >= 4.0 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled {
            return;
        }
        let Some(outer) = node.as_call_node() else { return };
        if !matches!(outer.name().as_slice(), b"find" | b"detect") {
            return;
        }
        if !is_bare_or_symbol_block_pass(&outer) {
            return;
        }
        let Some(receiver) = outer.receiver() else { return };
        let Some(inner) = receiver.as_call_node() else { return };
        if !matches!(inner.name().as_slice(), b"reverse" | b"reverse_each") {
            return;
        }

        let Some(inner_message) = inner.message_loc() else { return };
        let Some(outer_message) = outer.message_loc() else { return };
        let span = Span::new(inner_message.span().start, outer_message.span().end);

        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, b"rfind".to_vec())],
            },
        );
    }
}

/// RuboCop's `(block_pass sym)?`: no arguments at all, or exactly one
/// `&:symbol` block-pass argument.
fn is_bare_or_symbol_block_pass(call: &ruby_ast::node::CallNode<'_>) -> bool {
    let Some(args) = call.arguments() else { return true };
    let args = args.arguments();
    if args.len() != 1 {
        return false;
    }
    let Some(arg) = args.first() else { return false };
    let Some(block_pass) = arg.as_block_argument_node() else { return false };
    block_pass.expression().is_some_and(|e| e.as_symbol_node().is_some())
}
