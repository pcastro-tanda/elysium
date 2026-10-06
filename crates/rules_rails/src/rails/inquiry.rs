//! `Rails/Inquiry`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/inquiry.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "Prefer Ruby's comparison operators over Active Support's `inquiry`.";

/// Prefer Ruby's comparison operators over Active Support's `Array#inquiry` and `String#inquiry`.
#[derive(Debug, Clone)]
pub struct Inquiry;

impl Rule for Inquiry {
    const META: RuleMeta = RuleMeta {
        name: "Rails/Inquiry",
        department: Department::Rails,
        summary: "Prefer Ruby's comparison operators over Active Support's `Array#inquiry` and `String#inquiry`.",
        explanation: "Checks that Active Support's `inquiry` method is not used.\n\n```ruby\n# bad - String#inquiry\nruby = 'two'.inquiry\nruby.two?\n\n# good\nruby = 'two'\nruby == 'two'\n\n# bad - Array#inquiry\npets = %w(cat dog).inquiry\npets.gopher?\n\n# good\npets = %w(cat dog)\npets.include? 'cat'\n```",
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
        if call.name().as_slice() != b"inquiry" {
            return;
        }
        // `node.arguments.empty?`: a `&block` argument is an argument.
        if call.arguments().is_some()
            || call.block().is_some_and(|b| b.as_block_argument_node().is_some())
        {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !matches!(receiver.kind(), NodeKind::StringNode | NodeKind::ArrayNode) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        ctx.report(&Self::META, selector.span(), MSG);
    }
}
