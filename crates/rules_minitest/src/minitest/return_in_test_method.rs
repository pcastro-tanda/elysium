//! `Minitest/ReturnInTestMethod`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/return_in_test_method.rb` (with its
//! `MinitestExplorationHelpers` and `DefNode`/`VisibilityHelp` mixins).

use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, CallNode, ClassNode, DefNode};
use ruby_ast::{for_each_child, walk, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::SourceFile;
use ruby_source::Span;

const MSG: &str = "Use `skip` instead of `return`.";

/// Enforces the use of `skip` instead of `return` in test methods.
#[derive(Debug, Clone)]
pub struct ReturnInTestMethod;

impl Rule for ReturnInTestMethod {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/ReturnInTestMethod",
        department: Department::Minitest,
        summary: "Enforces the use of `skip` instead of `return` in test methods.",
        explanation: "Enforces the use of `skip` instead of `return` in test methods.\n\n```ruby\n# bad\ndef test_something\n  return if condition?\n  assert_equal(42, something)\nend\n\n# good\ndef test_something\n  skip if condition?\n  assert_equal(42, something)\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ProgramNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let mut visitor = Returns {
            stack: Vec::new(),
            map: non_public_map(node),
            source: ctx.source(),
            offenses: Vec::new(),
        };
        walk(node, &mut visitor);
        for span in visitor.offenses {
            ctx.report_with_fix(
                &Self::META,
                span,
                MSG,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(span, b"skip".to_vec())],
                },
            );
        }
    }
}

struct Returns<'a, 'pr> {
    stack: Vec<Node<'pr>>,
    map: HashMap<Key, bool>,
    source: &'a SourceFile,
    offenses: Vec<Span>,
}

impl<'pr> Visitor<'pr> for Returns<'_, 'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        if node.as_return_node().is_some()
            && (0..self.stack.len()).any(|index| self.test_case(index))
            && !self.inside_block()
        {
            self.offenses.push(node.span());
        }
        self.stack.push(*node);
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.stack.pop();
    }
}

impl Returns<'_, '_> {
    /// `test_case?` for the ancestor at `index`.
    fn test_case(&self, index: usize) -> bool {
        let node = &self.stack[index];
        let is_test_case = match node.as_def_node() {
            Some(def) => def.receiver().is_none() && test_method(&def, &self.map, true),
            None => test_block(node).is_some(),
        };
        if !is_test_case {
            return false;
        }
        let class = self.stack[..index].iter().rev().find_map(Node::as_class_node);
        class.is_some_and(|class| test_class(&class, self.source))
    }

    /// `node.ancestors.any?(&:any_block_type?)`
    fn inside_block(&self) -> bool {
        self.stack.iter().any(|ancestor| {
            ancestor.as_lambda_node().is_some()
                || ancestor
                    .as_call_node()
                    .is_some_and(|call| call.block().is_some_and(|b| b.as_block_node().is_some()))
        })
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

/// `test_class?`
fn test_class(class: &ClassNode<'_>, source: &SourceFile) -> bool {
    class.superclass().is_some() && source.slice(class.constant_path().span()).ends_with(b"Test")
}

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
