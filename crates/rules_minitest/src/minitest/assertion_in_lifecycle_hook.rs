//! `Minitest/AssertionInLifecycleHook`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assertion_in_lifecycle_hook.rb` (with its
//! `MinitestExplorationHelpers` mixin).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::{CallNode, ClassNode, DefNode};
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};

const VALUE_MATCHERS: &[&[u8]] = &[
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

const BLOCK_MATCHERS: &[&[u8]] = &[
    b"must_output",
    b"must_pattern_match",
    b"must_raise",
    b"must_be_silent",
    b"must_throw",
    b"wont_pattern_match",
];

const ASSERTION_PREFIXES: &[&[u8]] = &[b"assert", b"refute"];

const LIFECYCLE_HOOK_METHODS: &[&[u8]] = &[
    b"before_setup",
    b"setup",
    b"after_setup",
    b"before_teardown",
    b"teardown",
    b"after_teardown",
];

/// This cop checks for usage of assertions in lifecycle hooks.
#[derive(Debug, Clone)]
pub struct AssertionInLifecycleHook;

impl Rule for AssertionInLifecycleHook {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertionInLifecycleHook",
        department: Department::Minitest,
        summary: "This cop checks for usage of assertions in lifecycle hooks.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(class) = node.as_class_node() else { return };
        if !test_class(&class, ctx) {
            return;
        }

        for hook in class_def_nodes(&class)
            .into_iter()
            .filter(|def| LIFECYCLE_HOOK_METHODS.contains(&def.name().as_slice()))
        {
            let hook_name = String::from_utf8_lossy(hook.name().as_slice()).into_owned();
            each_descendant(&hook.as_node(), &mut |descendant: &Node<'_>| {
                let Some(call) = descendant.as_call_node() else { return };
                if call.is_safe_navigation() || !assertion_method(&call) {
                    return;
                }
                let message = format!(
                    "Do not use `{}` in `{hook_name}` hook.",
                    String::from_utf8_lossy(call.name().as_slice()),
                );
                ctx.report(&Self::META, call_span_excluding_block(&call), message);
            });
        }
    }
}

/// `class_node.parent_class && class_node.identifier.source.end_with?('Test')`.
fn test_class(class: &ClassNode<'_>, ctx: &Context<'_>) -> bool {
    class.superclass().is_some() && ctx.text(class.constant_path().span()).ends_with(b"Test")
}

/// `class_def_nodes`: the `def`s that are children of the class body (a lone
/// body node is one itself, or the node whose children are searched).
fn class_def_nodes<'pr>(class: &ClassNode<'pr>) -> Vec<DefNode<'pr>> {
    let Some(body) = class.body() else { return Vec::new() };
    if let Some(statements) = body.as_statements_node() {
        let list: Vec<Node<'pr>> = statements.body().iter().collect();
        return match list.as_slice() {
            [single] => single_child_defs(single),
            _ => list
                .iter()
                .filter_map(|stmt| stmt.as_def_node().filter(|def| def.receiver().is_none()))
                .collect(),
        };
    }
    // `rescue`/`ensure` bodies: only a lone `def` in the protected body is a child.
    if let Some(begin) = body.as_begin_node() {
        if let Some(statements) = begin.statements() {
            let list: Vec<Node<'pr>> = statements.body().iter().collect();
            if let [single] = list.as_slice() {
                return single
                    .as_def_node()
                    .filter(|def| def.receiver().is_none())
                    .into_iter()
                    .collect();
            }
        }
    }
    Vec::new()
}

/// `class_def.def_type? ? [class_def] : class_def.each_child_node(:def)` for a
/// class body of exactly one statement.
fn single_child_defs<'pr>(node: &Node<'pr>) -> Vec<DefNode<'pr>> {
    if let Some(def) = node.as_def_node() {
        return if def.receiver().is_none() { vec![def] } else { Vec::new() };
    }
    let Some(call) = node.as_call_node() else { return Vec::new() };
    let mut defs = Vec::new();
    let mut push = |candidate: &Node<'pr>| {
        if let Some(def) = candidate.as_def_node().filter(|def| def.receiver().is_none()) {
            defs.push(def);
        }
    };
    if let Some(block) = call.block().and_then(|block| block.as_block_node()) {
        // A `block` node's children are the send, the arguments and the body.
        if let Some(body) = block.body().and_then(|body| body.as_statements_node()) {
            let list: Vec<Node<'pr>> = body.body().iter().collect();
            if let [single] = list.as_slice() {
                push(single);
            }
        }
        return defs;
    }
    if let Some(receiver) = call.receiver() {
        push(&receiver);
    }
    if let Some(arguments) = call.arguments() {
        let list: Vec<Node<'pr>> = arguments.arguments().iter().collect();
        for argument in &list {
            push(argument);
        }
    }
    defs
}

/// `assertion_method?` for a `send` node.
fn assertion_method(call: &CallNode<'_>) -> bool {
    let name = call.name();
    let name = name.as_slice();
    let prefixed = call.receiver().is_none()
        && ASSERTION_PREFIXES.iter().any(|prefix| name.starts_with(prefix));
    prefixed || name == b"flunk" || VALUE_MATCHERS.contains(&name) || BLOCK_MATCHERS.contains(&name)
}
