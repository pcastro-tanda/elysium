//! `Minitest/UnreachableAssertion`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/unreachable_assertion.rb` (with its
//! `MinitestExplorationHelpers#assertion_method?`).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// This cop checks for an `assert_raises` block containing any unreachable assertions.
#[derive(Debug, Clone)]
pub struct UnreachableAssertion;

impl Rule for UnreachableAssertion {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/UnreachableAssertion",
        department: Department::Minitest,
        summary: "This cop checks for an `assert_raises` block containing any unreachable assertions.",
        explanation: "Checks for `assert_raises` has an assertion method at the bottom of block because the assertion will be never reached.\n\n```ruby\n# bad\nassert_raises FooError do\n  obj.occur_error\n  assert_equal('foo', obj.bar) # Never asserted.\nend\n\n# good\nassert_raises FooError do\n  obj.occur_error\nend\nassert_equal('foo', obj.bar)\n```",
        enabled_by_default: false,
        severity: Severity::Warning,
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
        if call.name().as_slice() != b"assert_raises" {
            return;
        }
        let Some(block) = call.block().and_then(|block| block.as_block_node()) else { return };
        // `on_block` only: not `numblock`/`itblock`.
        if block.parameters().is_some_and(|parameters| {
            parameters.as_numbered_parameters_node().is_some()
                || parameters.as_it_parameters_node().is_some()
        }) {
            return;
        }
        let Some(body) = block.body() else { return };
        // `body.begin_type?`: more than one statement.
        let Some(statements) = body.as_statements_node() else { return };
        let list: Vec<Node<'_>> = statements.body().iter().collect();
        if list.len() < 2 {
            return;
        }
        let Some(last_node) = list.last() else { return };
        let Some(last_call) = send_node(last_node) else { return };
        if !assertion_method(&last_call) {
            return;
        }
        let name = String::from_utf8_lossy(last_call.name().as_slice()).into_owned();
        ctx.report(&Self::META, last_node.span(), format!("Unreachable `{name}` detected."));
    }
}

const VALUE_MATCHERS: [&[u8]; 28] = [
    b"must_be_empty",
    b"must_equal",
    b"must_be_close_to",
    b"must_be_within_delta",
    b"must_be_within_epsilon",
    b"must_include",
    b"must_be_instance_of",
    b"must_be_kind_of",
    b"must_match",
    b"must_be_nil",
    b"must_be",
    b"must_respond_to",
    b"must_be_same_as",
    b"path_must_exist",
    b"path_wont_exist",
    b"wont_be_empty",
    b"wont_equal",
    b"wont_be_close_to",
    b"wont_be_within_delta",
    b"wont_be_within_epsilon",
    b"wont_include",
    b"wont_be_instance_of",
    b"wont_be_kind_of",
    b"wont_match",
    b"wont_be_nil",
    b"wont_be",
    b"wont_respond_to",
    b"wont_be_same_as",
];

const BLOCK_MATCHERS: [&[u8]; 6] = [
    b"must_output",
    b"must_pattern_match",
    b"must_raise",
    b"must_be_silent",
    b"must_throw",
    b"wont_pattern_match",
];

/// A `send` node: not `csend`, not wrapped in a block.
fn send_node<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    let wrapped = call.block().is_some_and(|block| block.as_block_node().is_some());
    (!call.is_safe_navigation() && !wrapped).then_some(call)
}

/// `assertion_method?` for a `send` node.
fn assertion_method(call: &CallNode<'_>) -> bool {
    let name = call.name();
    let name = name.as_slice();
    let prefix_method =
        call.receiver().is_none() && (name.starts_with(b"assert") || name.starts_with(b"refute"));
    prefix_method
        || name == b"flunk"
        || VALUE_MATCHERS.contains(&name)
        || BLOCK_MATCHERS.contains(&name)
}
