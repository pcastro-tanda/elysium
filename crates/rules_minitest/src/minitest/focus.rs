//! `Minitest/Focus`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/focus.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Remove `focus` from tests.";

/// Checks for focused tests.
#[derive(Debug, Clone)]
pub struct Focus;

impl Rule for Focus {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/Focus",
        department: Department::Minitest,
        summary: "Checks for focused tests.",
        explanation: "Enforces tests are not focused.\n\n```ruby\n# bad\nfocus test 'foo' do\nend\n\n# bad\nfocus\ntest 'foo' do\nend\n\n# good\ntest 'foo' do\nend\n```",
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
        // RESTRICT_ON_SEND = [:focus]; `on_send` does not fire for `&.`.
        if call.name().as_slice() != b"focus" || call.is_safe_navigation() {
            return;
        }
        if call.receiver().is_some() {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let selector = selector.span();

        // whitequark lists a `&block` argument among `arguments`.
        let first_argument = call
            .arguments()
            .and_then(|args| args.arguments().iter().next())
            .or_else(|| call.block().filter(|block| block.as_block_argument_node().is_some()));
        let range = match first_argument {
            None => ctx.whole_lines(call_span_excluding_block(&call)),
            Some(first) => Span::new(selector.start, first.span().start),
        };
        ctx.report_with_fix(
            &Self::META,
            selector,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(range)] },
        );
    }
}
