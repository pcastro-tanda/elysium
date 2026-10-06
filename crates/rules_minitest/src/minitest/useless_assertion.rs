//! `Minitest/UselessAssertion`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/useless_assertion.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Useless assertion detected.";

const SINGLE_ASSERTION_ARGUMENT_METHODS: &[&[u8]] = &[
    b"assert",
    b"refute",
    b"assert_nil",
    b"refute_nil",
    b"assert_not",
    b"assert_empty",
    b"refute_empty",
];
const TWO_ASSERTION_ARGUMENTS_METHODS: &[&[u8]] = &[
    b"assert_equal",
    b"refute_equal",
    b"assert_in_delta",
    b"refute_in_delta",
    b"assert_in_epsilon",
    b"refute_in_epsilon",
    b"assert_same",
    b"refute_same",
];

/// Detects useless assertions (assertions that either always pass or always
/// fail).
#[derive(Debug, Clone)]
pub struct UselessAssertion;

impl Rule for UselessAssertion {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/UselessAssertion",
        department: Department::Minitest,
        summary: "Detects useless assertions (assertions that either always pass or always fail).",
        explanation: "Detects useless assertions (assertions that either always pass or always \
                      fail).\n\n```ruby\n# bad\nassert true\nassert_equal @foo, @foo\n\
                      assert_nil [foo, bar]\n\n# good\nassert something\n\
                      assert_equal foo, bar\nassert_nil foo\nassert false, \"My message\"\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
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
        // `on_send` does not fire for `csend`; `return if node.receiver`.
        if call.receiver().is_some() || call.is_safe_navigation() {
            return;
        }
        if offense(&call, ctx) {
            ctx.report(&Self::META, call_span_excluding_block(&call), MSG);
        }
    }
}

fn offense(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    let arguments = argument_list(call);
    // `expected, actual, = node.arguments`
    let expected = arguments.first();
    let actual = arguments.get(1);
    let name = call.name();
    let name = name.as_slice();

    if SINGLE_ASSERTION_ARGUMENT_METHODS.contains(&name) {
        actual.is_none() && expected.is_some_and(|e| is_literal(e) && !is_xstr(e))
    } else if TWO_ASSERTION_ARGUMENTS_METHODS.contains(&name) {
        let (Some(expected), Some(actual)) = (expected, actual) else { return false };
        if ctx.text(expected.span()) != ctx.text(actual.span()) {
            return false;
        }
        (is_variable(expected) && is_variable(actual))
            || (is_empty_composite(expected) && is_empty_composite(actual))
    } else if name == b"assert_includes" || name == b"refute_includes" {
        expected.is_some_and(is_empty_composite)
    } else if name == b"assert_silent" {
        // `node.parent` is the block that the call is the send of.
        call.block()
            .and_then(|block| block.as_block_node())
            .is_some_and(|block| block.body().is_none())
    } else {
        false
    }
}

/// `Node#literal?`.
fn is_literal(node: &Node<'_>) -> bool {
    node.as_string_node().is_some()
        || node.as_interpolated_string_node().is_some()
        || node.as_x_string_node().is_some()
        || node.as_interpolated_x_string_node().is_some()
        || node.as_integer_node().is_some()
        || node.as_float_node().is_some()
        || node.as_symbol_node().is_some()
        || node.as_interpolated_symbol_node().is_some()
        || node.as_array_node().is_some()
        || node.as_hash_node().is_some()
        || node.as_keyword_hash_node().is_some()
        || node.as_regular_expression_node().is_some()
        || node.as_interpolated_regular_expression_node().is_some()
        || node.as_true_node().is_some()
        || node.as_false_node().is_some()
        || node.as_nil_node().is_some()
        || node.as_range_node().is_some()
        || node.as_imaginary_node().is_some()
        || node.as_rational_node().is_some()
        || node.as_source_file_node().is_some()
        || node.as_source_line_node().is_some()
}

fn is_xstr(node: &Node<'_>) -> bool {
    node.as_x_string_node().is_some() || node.as_interpolated_x_string_node().is_some()
}

/// `Node#variable?`: `lvar`, `ivar`, `cvar`, `gvar`.
fn is_variable(node: &Node<'_>) -> bool {
    node.as_local_variable_read_node().is_some()
        || node.as_instance_variable_read_node().is_some()
        || node.as_class_variable_read_node().is_some()
        || node.as_global_variable_read_node().is_some()
}

/// `empty_composite?`: an empty `str`, `array`, or `hash`.
fn is_empty_composite(node: &Node<'_>) -> bool {
    if let Some(string) = node.as_string_node() {
        return string.unescaped().is_empty();
    }
    if let Some(array) = node.as_array_node() {
        return array.elements().iter().next().is_none();
    }
    if let Some(hash) = node.as_hash_node() {
        return hash.elements().iter().next().is_none();
    }
    false
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
