//! `Style/Send`, ported from RuboCop's
//! `lib/rubocop/cop/style/send.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

const MSG: &str = "Prefer `Object#__send__` or `Object#public_send` to `send`.";

/// Prefer `Object#__send__` or `Object#public_send` to `send`, as `send` may overlap with existing methods.
#[derive(Debug, Clone)]
pub struct Send;

impl Rule for Send {
    const META: RuleMeta = RuleMeta {
        name: "Style/Send",
        department: Department::Style,
        summary: "Prefer `Object#__send__` or `Object#public_send` to `send`, as `send` may overlap with existing methods.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
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
        if call.name().as_slice() != b"send" {
            return;
        }
        if call.arguments().is_none() {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        ctx.report(&Self::META, selector.span(), MSG);
    }
}
