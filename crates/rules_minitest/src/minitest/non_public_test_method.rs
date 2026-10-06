//! `Minitest/NonPublicTestMethod`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/non_public_test_method.rb` (with its
//! `MinitestExplorationHelpers` and `DefNode`/`VisibilityHelp` mixins).

use std::collections::{BTreeSet, HashMap};

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::{BlockNode, CallNode, ClassNode, DefNode};
use ruby_ast::{each_descendant, for_each_child, walk, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

const MSG: &str = "Non `public` test method detected. Make it `public` for it to run.";

/// Detects non `public` (marked as `private` or `protected`) test methods.
/// Minitest runs only test methods which are `public`.
#[derive(Debug, Clone)]
pub struct NonPublicTestMethod;

impl Rule for NonPublicTestMethod {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/NonPublicTestMethod",
        department: Department::Minitest,
        summary: "Detects non `public` (marked as `private` or `protected`) test methods.",
        explanation: "Detects non `public` (marked as `private` or `protected`) test methods. Minitest runs only test methods which are `public`.\n\n```ruby\n# bad\nclass FooTest\n  private # or protected\n  def test_does_something\n    assert_equal 42, do_something\n  end\nend\n\n# good\nclass FooTest\n  def test_does_something\n    assert_equal 42, do_something\n  end\nend\n\n# good (not a test case name)\nclass FooTest\n  private # or protected\n  def does_something\n    assert_equal 42, do_something\n  end\nend\n\n# good (no assertions)\nclass FooTest\n  private # or protected\n  def test_does_something\n    do_something\n  end\nend\n```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ProgramNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let mut visitor = Classes { map: non_public_map(node), offenses: BTreeSet::new() };
        walk(node, &mut visitor);
        for (start, end) in visitor.offenses {
            ctx.report(&Self::META, Span::new(start, end), MSG);
        }
    }
}

struct Classes {
    map: HashMap<Key, bool>,
    offenses: BTreeSet<Key>,
}

impl<'pr> Visitor<'pr> for Classes {
    fn enter(&mut self, node: &Node<'pr>) {
        if node.as_class_node().is_none() {
            return;
        }
        for test_case in test_cases(node, &self.map, false) {
            let non_public = self.map.get(&key(&test_case.node)).copied().unwrap_or(false);
            if non_public && !assertions(test_case.body).is_empty() {
                self.offenses.insert(key(&test_case.node));
            }
        }
    }
}

/// The children of `node` the way whitequark lists them: a single-statement
/// `StatementsNode` is elided, `ArgumentsNode`s are flattened, and a call with
/// a block is the block node, whose body is its only interesting child.
fn effective_children<'pr>(node: &Node<'pr>) -> Vec<Node<'pr>> {
    let mut out = Vec::new();
    if let Some(parens) = node.as_parentheses_node() {
        if let Some(body) = parens.body() {
            match body.as_statements_node() {
                Some(statements) => out.extend(statements.body().iter()),
                None => out.push(body),
            }
        }
        return out;
    }
    if let Some(call) = node.as_call_node() {
        if let Some(block) = call.block().and_then(|block| block.as_block_node()) {
            if let Some(body) = block.body() {
                match body.as_statements_node() {
                    Some(statements) if statements.body().len() == 1 => {
                        out.extend(statements.body().iter());
                    }
                    _ => out.push(body),
                }
            }
            return out;
        }
    }
    for_each_child(node, |child| {
        if let Some(statements) = child.as_statements_node() {
            if statements.body().len() == 1 {
                out.extend(statements.body().iter());
                return;
            }
        }
        if let Some(arguments) = child.as_arguments_node() {
            out.extend(arguments.arguments().iter());
            return;
        }
        out.push(*child);
    });
    out
}

/// A node's place in the tree the way whitequark sees it.
struct Position<'pr> {
    /// The parent, when it is a real node (not an implicit `begin`).
    parent: Option<Node<'pr>>,
    /// The parent's children, the node itself included.
    children: Vec<Node<'pr>>,
    /// Where the node is among `children`.
    index: Option<usize>,
}

impl<'pr> Position<'pr> {
    fn left(&self) -> &[Node<'pr>] {
        self.index.map_or(&[], |index| &self.children[..index])
    }

    fn right(&self) -> &[Node<'pr>] {
        self.index.map_or(&[], |index| &self.children[index + 1..])
    }
}

fn position<'pr>(ancestors: &[Node<'pr>], node: &Node<'pr>) -> Position<'pr> {
    let count = ancestors.len();
    let (exists, parent, children) = match ancestors.last() {
        None => (false, None, Vec::new()),
        Some(innermost) => {
            if let Some(statements) = innermost.as_statements_node() {
                let siblings: Vec<Node<'pr>> = statements.body().iter().collect();
                if siblings.len() > 1 {
                    (true, None, siblings)
                } else {
                    let parent = count
                        .checked_sub(2)
                        .map(|i| ancestors[i])
                        .filter(|parent| parent.as_program_node().is_none());
                    let children = parent.as_ref().map(effective_children).unwrap_or_default();
                    (parent.is_some(), parent, children)
                }
            } else if innermost.as_arguments_node().is_some() {
                let parent = count.checked_sub(2).map(|i| ancestors[i]);
                let children = parent.as_ref().map(effective_children).unwrap_or_default();
                (true, parent, children)
            } else {
                (true, Some(*innermost), effective_children(innermost))
            }
        }
    };
    let span = node.span();
    let index = children.iter().position(|child| {
        let child_span = child.span();
        child_span.start == span.start && child_span.end == span.end && child.kind() == node.kind()
    });
    let _ = exists;
    Position { parent, children, index }
}

const VISIBILITY_SCOPES: [&[u8]; 3] = [b"private", b"protected", b"public"];

/// `(send nil? VISIBILITY_SCOPES ...)` with exactly the given arguments
/// predicate; returns the visibility name.
fn visibility_call<'pr>(node: &Node<'pr>) -> Option<(CallNode<'pr>, Vec<Node<'pr>>)> {
    let call = node.as_call_node()?;
    if call.receiver().is_some()
        || call.block().is_some()
        || !VISIBILITY_SCOPES.contains(&call.name().as_slice())
    {
        return None;
    }
    let arguments =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    Some((call, arguments))
}

/// `(send nil? VISIBILITY_SCOPES)`
fn visibility_block(node: &Node<'_>) -> Option<Vec<u8>> {
    let (call, arguments) = visibility_call(node)?;
    arguments.is_empty().then(|| call.name().as_slice().to_vec())
}

/// `(send nil? VISIBILITY_SCOPES def)`
fn visibility_inline_on_def(node: &Node<'_>) -> Option<Vec<u8>> {
    let (call, arguments) = visibility_call(node)?;
    let [only] = arguments.as_slice() else { return None };
    only.as_def_node().filter(|def| def.receiver().is_none())?;
    Some(call.name().as_slice().to_vec())
}

/// `(send nil? VISIBILITY_SCOPES (sym %method_name))`
fn visibility_inline_on_method_name(node: &Node<'_>, method_name: &[u8]) -> Option<Vec<u8>> {
    let (call, arguments) = visibility_call(node)?;
    let [only] = arguments.as_slice() else { return None };
    (only.as_symbol_node()?.unescaped() == method_name).then(|| call.name().as_slice().to_vec())
}

/// `(send nil? {:private :protected :private_class_method} (any_def ...))`
fn non_public_modifier(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.receiver().is_some()
        || call.block().is_some()
        || !matches!(call.name().as_slice(), b"private" | b"protected" | b"private_class_method")
    {
        return false;
    }
    let Some(arguments) = call.arguments() else { return false };
    let arguments: Vec<Node<'_>> = arguments.arguments().iter().collect();
    matches!(arguments.as_slice(), [only] if only.as_def_node().is_some())
}

/// `VisibilityHelp#node_visibility`.
fn node_visibility(node: &Node<'_>, position: &Position<'_>) -> Vec<u8> {
    if let Some(def) = node.as_def_node() {
        if let Some(visibility) = position.parent.as_ref().and_then(visibility_inline_on_def) {
            return visibility;
        }
        let name = def.name();
        if let Some(visibility) = position
            .right()
            .iter()
            .rev()
            .find_map(|sibling| visibility_inline_on_method_name(sibling, name.as_slice()))
        {
            return visibility;
        }
    }
    if let Some(visibility) = position.left().iter().rev().find_map(visibility_block) {
        return visibility;
    }
    b"public".to_vec()
}

/// `DefNode#non_public?`.
fn non_public(node: &Node<'_>, position: &Position<'_>) -> bool {
    position.parent.as_ref().is_some_and(non_public_modifier)
        || node_visibility(node, position) != b"public"
}

type Key = (u32, u32);

fn key(node: &Node<'_>) -> Key {
    let span = node.span();
    (span.start, span.end)
}

/// Whether each receiverless `def` and each call with a block is
/// `non_public?`, keyed by the node's range.
struct NonPublicMap<'pr> {
    stack: Vec<Node<'pr>>,
    map: HashMap<Key, bool>,
}

impl<'pr> Visitor<'pr> for NonPublicMap<'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        let tracked = node.as_def_node().is_some_and(|def| def.receiver().is_none())
            || node
                .as_call_node()
                .is_some_and(|call| call.block().is_some_and(|b| b.as_block_node().is_some()));
        if tracked {
            let position = position(&self.stack, node);
            self.map.insert(key(node), non_public(node, &position));
        }
        self.stack.push(*node);
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.stack.pop();
    }
}

fn non_public_map(root: &Node<'_>) -> HashMap<Key, bool> {
    let mut visitor = NonPublicMap { stack: Vec::new(), map: HashMap::new() };
    walk(root, &mut visitor);
    visitor.map
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

/// `test_method?`
fn test_method(def: &DefNode<'_>, non_public: &HashMap<Key, bool>, visibility_check: bool) -> bool {
    if visibility_check && non_public.get(&key(&def.as_node())).copied().unwrap_or(false) {
        return false;
    }
    def.name().as_slice().starts_with(b"test_") && def.parameters().is_none()
}

/// A `block` node (not `numblock`/`itblock`) whose call is `test` or `it`
/// (`test_block?`).
fn test_block<'pr>(node: &Node<'pr>) -> Option<BlockNode<'pr>> {
    let call = node.as_call_node()?;
    let block = call.block()?.as_block_node()?;
    if block.parameters().is_some_and(|parameters| {
        parameters.as_numbered_parameters_node().is_some()
            || parameters.as_it_parameters_node().is_some()
    }) {
        return None;
    }
    matches!(call.name().as_slice(), b"test" | b"it").then_some(block)
}

/// `class_def_nodes`
fn class_def_nodes<'pr>(class: &ClassNode<'pr>) -> Vec<DefNode<'pr>> {
    let Some(body) = class.body() else { return Vec::new() };
    let nodes: Vec<Node<'pr>> = match body.as_statements_node() {
        Some(statements) => {
            let list: Vec<Node<'pr>> = statements.body().iter().collect();
            match list.as_slice() {
                [only] => {
                    if only.as_def_node().is_some_and(|def| def.receiver().is_none()) {
                        list
                    } else {
                        effective_children(only)
                    }
                }
                _ => list,
            }
        }
        None => effective_children(&body),
    };
    nodes.iter().filter_map(Node::as_def_node).filter(|def| def.receiver().is_none()).collect()
}

struct TestCase<'pr> {
    node: Node<'pr>,
    body: Option<Node<'pr>>,
}

/// `test_cases`
fn test_cases<'pr>(
    class_node: &Node<'pr>,
    non_public: &HashMap<Key, bool>,
    visibility_check: bool,
) -> Vec<TestCase<'pr>> {
    let mut cases = Vec::new();
    if let Some(class) = class_node.as_class_node() {
        for def in class_def_nodes(&class) {
            if test_method(&def, non_public, visibility_check) {
                cases.push(TestCase { node: def.as_node(), body: def.body() });
            }
        }
    }
    each_descendant(class_node, &mut |node| {
        if let Some(block) = test_block(node) {
            cases.push(TestCase { node: *node, body: block.body() });
        }
    });
    cases
}

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

/// `assertions`
fn assertions<'pr>(body: Option<Node<'pr>>) -> Vec<CallNode<'pr>> {
    let Some(method_def) = body else { return Vec::new() };
    let nodes: Vec<Node<'pr>> = match method_def.as_statements_node() {
        Some(statements) => {
            let list: Vec<Node<'pr>> = statements.body().iter().collect();
            match list.as_slice() {
                [only] if send_node(only).is_some() => list,
                [only] => effective_children(only),
                _ => list,
            }
        }
        None => effective_children(&method_def),
    };
    nodes.iter().filter_map(send_node).filter(assertion_method).collect()
}
