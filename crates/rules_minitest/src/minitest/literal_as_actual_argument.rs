//! `Minitest/LiteralAsActualArgument`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/literal_as_actual_argument.rb` (with its
//! `ArgumentRangeHelper` mixin).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, is_recursive_basic_literal};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Replace the literal with the first argument.";

/// Enforces correct order of expected and actual arguments for `assert_equal`.
#[derive(Debug, Clone)]
pub struct LiteralAsActualArgument;

impl Rule for LiteralAsActualArgument {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/LiteralAsActualArgument",
        department: Department::Minitest,
        summary: "This cop enforces correct order of `expected` and `actual` arguments for \
                  `assert_equal`.",
        explanation: "Enforces correct order of expected and actual arguments for \
                      `assert_equal`.\n\n```ruby\n# bad\nassert_equal foo, 2\n\
                      assert_equal foo, [1, 2]\nassert_equal foo, [1, 2], 'message'\n\n\
                      # good\nassert_equal 2, foo\nassert_equal [1, 2], foo\n\
                      assert_equal [1, 2], foo, 'message'\n```",
        enabled_by_default: false,
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
        if call.is_safe_navigation() || call.name().as_slice() != b"assert_equal" {
            return;
        }
        let arguments = argument_list(&call);
        let [expected, actual, ..] = arguments.as_slice() else { return };
        if !is_recursive_basic_literal(actual) || is_recursive_basic_literal(expected) {
            return;
        }

        // `all_arguments_range`.
        let range = Span::new(expected.span().start, arguments[arguments.len() - 1].span().end);

        let expected_source = ctx.text(expected.span()).to_vec();
        let actual_source = ctx.text(actual.span());
        let is_hash = actual.as_hash_node().is_some() || actual.as_keyword_hash_node().is_some();
        let new_actual_source = if actual.as_keyword_hash_node().is_some() {
            let mut wrapped = b"{".to_vec();
            wrapped.extend_from_slice(actual_source);
            wrapped.push(b'}');
            wrapped
        } else {
            actual_source.to_vec()
        };

        let mut edits = vec![
            Edit::replace(expected.span(), new_actual_source),
            Edit::replace(actual.span(), expected_source),
        ];
        if call.opening_loc().is_none() && is_hash {
            // `wrap_with_parentheses`.
            if let Some(selector) = call.message_loc() {
                edits.push(Edit::replace(
                    Span::new(selector.span().end, expected.span().start),
                    b"(".to_vec(),
                ));
                let end = call_span_excluding_block(&call)
                    .end
                    .max(arguments[arguments.len() - 1].span().end);
                edits.push(Edit::insert(end, b")".to_vec()));
            }
        }
        ctx.report_with_fix(
            &Self::META,
            range,
            MSG,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// The call's arguments the way whitequark lists a `send`'s: a `&block`
/// argument is one of them.
fn argument_list<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut args: Vec<Node<'pr>> =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block().filter(|block| block.as_block_argument_node().is_some()) {
        args.push(block);
    }
    args
}
