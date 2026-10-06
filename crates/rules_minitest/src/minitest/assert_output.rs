//! `Minitest/AssertOutput`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_output.rb` (with its
//! `MinitestExplorationHelpers` and rubocop's `DefNode`/`VisibilityHelp` mixins).

use std::collections::HashSet;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, walk, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

const OUTPUT_GLOBAL_VARIABLES: [&[u8]; 2] = [b"$stdout", b"$stderr"];

const VALUE_MATCHERS: [&[u8]; 29] = [
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
    b"flunk",
];

const BLOCK_MATCHERS: [&[u8]; 6] = [
    b"must_output",
    b"must_pattern_match",
    b"must_raise",
    b"must_be_silent",
    b"must_throw",
    b"wont_pattern_match",
];

/// Checks for opportunities to use `assert_output`.
#[derive(Debug, Clone)]
pub struct AssertOutput;

impl Rule for AssertOutput {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertOutput",
        department: Department::Minitest,
        summary: "This cop checks for opportunities to use `assert_output`.",
        explanation: "Checks for opportunities to use `assert_output`.\n\n```ruby\n# bad\n\
                      $stdout = StringIO.new\nputs object.method\n$stdout.rewind\n\
                      assert_match expected, $stdout.read\n\n# good\n\
                      assert_output(expected) { puts object.method }\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ProgramNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let mut finder =
            Finder { source: ctx.source().bytes(), stack: Vec::new(), offenses: Vec::new() };
        walk(node, &mut finder);
        let mut seen = HashSet::new();
        for (span, name) in finder.offenses {
            // `add_offense` ignores a second offense at the same location.
            if seen.insert(span) {
                let message = format!(
                    "Use `assert_output` instead of mutating {}.",
                    String::from_utf8_lossy(name)
                );
                ctx.report(&Self::META, span, message);
            }
        }
    }
}

struct Finder<'a, 'pr> {
    source: &'a [u8],
    stack: Vec<Node<'pr>>,
    offenses: Vec<(Span, &'static [u8])>,
}

impl<'pr> Visitor<'pr> for Finder<'_, 'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        if let Some(name) = gvasgn_name(node) {
            if let Some(gvar_name) = OUTPUT_GLOBAL_VARIABLES.iter().find(|n| **n == name) {
                self.check(gvar_name);
            }
        }
        self.stack.push(*node);
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.stack.pop();
    }
}

impl<'pr> Finder<'_, 'pr> {
    /// `on_gvasgn`, for the node about to be pushed onto `self.stack`.
    fn check(&mut self, gvar_name: &'static [u8]) {
        let Some(test_case) = (0..self.stack.len()).rev().find(|&i| self.test_case(i)) else {
            return;
        };
        let body = match &self.stack[test_case] {
            Node::DefNode { .. } => self.stack[test_case].as_def_node().and_then(|d| d.body()),
            _ => self.stack[test_case].as_block_node().and_then(|b| b.body()),
        };
        for call in assertions(body) {
            if assertion_method(&call) && references_gvar(&call, gvar_name) {
                self.offenses.push((assertion_span(&call), gvar_name));
            }
        }
    }

    /// `test_case?(self.stack[index])`.
    fn test_case(&self, index: usize) -> bool {
        let node = &self.stack[index];
        if let Some(def) = node.as_def_node() {
            let is_test_method = def.receiver().is_none()
                && def.name().as_slice().starts_with(b"test_")
                && def.parameters().is_none()
                && !self.non_public(index);
            if !is_test_method {
                return false;
            }
        } else if let Some(block) = node.as_block_node() {
            let named_test = index
                .checked_sub(1)
                .and_then(|parent| self.stack[parent].as_call_node())
                .is_some_and(|call| matches!(call.name().as_slice(), b"test" | b"it"));
            let numbered = block.parameters().is_some_and(|p| {
                p.as_numbered_parameters_node().is_some() || p.as_it_parameters_node().is_some()
            });
            if !named_test || numbered {
                return false;
            }
        } else {
            return false;
        }
        // `node.each_ancestor(:class).first`, then `test_class?`.
        self.stack[..index].iter().rev().find_map(Node::as_class_node).is_some_and(|class| {
            class.superclass().is_some()
                && self.source[class.constant_path().span().start as usize
                    ..class.constant_path().span().end as usize]
                    .ends_with(b"Test")
        })
    }

    /// `non_public?(def_node)` of rubocop's `DefNode` mixin.
    fn non_public(&self, index: usize) -> bool {
        // non_public_modifier?(node.parent):
        //   (send nil? {:private :protected :private_class_method} (any_def ...))
        if let Some(call) = self.inline_modifier(index) {
            if matches!(call.name().as_slice(), b"private" | b"protected" | b"private_class_method")
            {
                return true;
            }
        }
        self.node_visibility(index) != b"public"
    }

    /// The call `<modifier> def ...` whose only argument is `stack[index]`.
    fn inline_modifier(&self, index: usize) -> Option<CallNode<'pr>> {
        let arguments = self.stack[index.checked_sub(1)?].as_arguments_node()?;
        if arguments.arguments().len() != 1 {
            return None;
        }
        let call = self.stack[index.checked_sub(2)?].as_call_node()?;
        (call.receiver().is_none() && call.block().is_none()).then_some(call)
    }

    /// `node_visibility(node)` of rubocop's `VisibilityHelp`.
    fn node_visibility(&self, index: usize) -> &[u8] {
        let def_name = self.stack[index].as_def_node().map(|d| d.name());
        // node_visibility_from_visibility_inline_on_def
        if let Some(call) = self.inline_modifier(index) {
            let name = call.name();
            if is_visibility_scope(name.as_slice()) {
                return scope_name(name.as_slice());
            }
        }
        let siblings = self.siblings(index);
        // node_visibility_from_visibility_inline_on_method_name
        if let (Some(def_name), Some((position, list))) = (def_name, &siblings) {
            let found = list[position + 1..].iter().rev().find_map(|sibling| {
                let call = sibling.as_call_node()?;
                if call.receiver().is_some() || !is_visibility_scope(call.name().as_slice()) {
                    return None;
                }
                let arguments = call.arguments()?.arguments();
                if arguments.len() != 1 {
                    return None;
                }
                let symbol = arguments.first()?.as_symbol_node()?.unescaped().to_vec();
                (symbol == def_name.as_slice()).then(|| scope_name(call.name().as_slice()))
            });
            if let Some(found) = found {
                return found;
            }
        }
        // node_visibility_from_visibility_block
        if let Some((position, list)) = &siblings {
            let found = list[..*position].iter().rev().find_map(|sibling| {
                let call = sibling.as_call_node()?;
                (call.receiver().is_none()
                    && call.arguments().is_none()
                    && call.block().is_none()
                    && is_visibility_scope(call.name().as_slice()))
                .then(|| scope_name(call.name().as_slice()))
            });
            if let Some(found) = found {
                return found;
            }
        }
        b"public"
    }

    /// The statements `stack[index]` sits among, and its position there.
    fn siblings(&self, index: usize) -> Option<(usize, Vec<Node<'pr>>)> {
        let parent = self.stack[index.checked_sub(1)?].as_statements_node()?;
        let list: Vec<Node<'pr>> = parent.body().iter().collect();
        let start = self.stack[index].span().start;
        let position = list.iter().position(|n| n.span().start == start)?;
        Some((position, list))
    }
}

fn is_visibility_scope(name: &[u8]) -> bool {
    matches!(name, b"private" | b"protected" | b"public")
}

fn scope_name(name: &[u8]) -> &'static [u8] {
    match name {
        b"private" => b"private",
        b"protected" => b"protected",
        _ => b"public",
    }
}

/// The name of a `gvasgn` (including the one nested in `op_asgn`/`or_asgn`/
/// `and_asgn`, `masgn`, `for` and `rescue =>` targets).
fn gvasgn_name<'a>(node: &Node<'a>) -> Option<&'a [u8]> {
    match node {
        Node::GlobalVariableWriteNode { .. } => {
            node.as_global_variable_write_node().map(|n| n.name().as_slice())
        }
        Node::GlobalVariableOperatorWriteNode { .. } => {
            node.as_global_variable_operator_write_node().map(|n| n.name().as_slice())
        }
        Node::GlobalVariableOrWriteNode { .. } => {
            node.as_global_variable_or_write_node().map(|n| n.name().as_slice())
        }
        Node::GlobalVariableAndWriteNode { .. } => {
            node.as_global_variable_and_write_node().map(|n| n.name().as_slice())
        }
        Node::GlobalVariableTargetNode { .. } => {
            node.as_global_variable_target_node().map(|n| n.name().as_slice())
        }
        _ => None,
    }
}

/// Whitequark `send` node: neither `&.` nor carrying a literal block.
fn is_send(call: &CallNode<'_>) -> bool {
    !call.is_safe_navigation() && call.block().is_none_or(|b| b.as_block_node().is_none())
}

/// `assertions(def_node)`'s `send_nodes`, before the `assertion_method?` filter:
/// `method_def.send_type? ? [method_def] : method_def.each_child_node(:send)`
/// on the whitequark shape of `body`.
fn assertions(body: Option<Node<'_>>) -> Vec<CallNode<'_>> {
    let Some(body) = body else { return Vec::new() };
    if let Some(statements) = body.as_statements_node() {
        let list: Vec<Node<'_>> = statements.body().iter().collect();
        return match list.as_slice() {
            [only] => single_statement(only),
            // `begin` node: its `send` children.
            _ => list.iter().filter_map(Node::as_call_node).filter(is_send).collect(),
        };
    }
    if let Some(begin) = body.as_begin_node() {
        // A body with `rescue`/`ensure` (no `begin` keyword).
        return rescue_ensure_sends(&begin);
    }
    single_statement(&body)
}

/// The single statement of `statements`, elided to itself the way whitequark
/// elides a one-statement `begin`.
fn only_statement<'pr>(
    statements: Option<ruby_ast::node::StatementsNode<'pr>>,
) -> Option<Node<'pr>> {
    let list: Vec<Node<'pr>> = statements?.body().iter().collect();
    match list.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

/// `send` children of the `(ensure ...)`/`(rescue ...)` node of a body with
/// `rescue`/`ensure`.
fn rescue_ensure_sends<'pr>(begin: &ruby_ast::node::BeginNode<'pr>) -> Vec<CallNode<'pr>> {
    let as_send = |node: Option<Node<'pr>>| node.and_then(|n| n.as_call_node()).filter(is_send);
    let mut found = Vec::new();
    if let Some(ensure) = begin.ensure_clause() {
        // (ensure <rescue or body> <ensure body>)
        if begin.rescue_clause().is_none() && begin.else_clause().is_none() {
            found.extend(as_send(only_statement(begin.statements())));
        }
        found.extend(as_send(only_statement(ensure.statements())));
    } else {
        // (rescue <body> (resbody ...)... <else body>)
        found.extend(as_send(only_statement(begin.statements())));
        if let Some(else_clause) = begin.else_clause() {
            found.extend(as_send(only_statement(else_clause.statements())));
        }
    }
    found
}

/// `method_def.send_type? ? [method_def] : method_def.each_child_node(:send)`
/// for a body that is a single statement.
fn single_statement<'pr>(statement: &Node<'pr>) -> Vec<CallNode<'pr>> {
    let mut found = Vec::new();
    match statement {
        Node::CallNode { .. } => {
            let Some(call) = statement.as_call_node() else { return found };
            if let Some(block) = call.block().and_then(|b| b.as_block_node()) {
                // (block (send ...) args body): the `send`, the single-statement body.
                if !call.is_safe_navigation() {
                    found.push(call);
                }
                found.extend(
                    only_statement(block.body().and_then(|b| b.as_statements_node()))
                        .and_then(|n| n.as_call_node())
                        .filter(is_send),
                );
            } else if call.is_safe_navigation() {
                // `csend`: receiver and arguments are its children.
                found.extend(call.receiver().and_then(|r| r.as_call_node()).filter(is_send));
                if let Some(arguments) = call.arguments() {
                    found.extend(
                        arguments
                            .arguments()
                            .iter()
                            .filter_map(|a| Node::as_call_node(&a))
                            .filter(is_send),
                    );
                }
            } else {
                found.push(call);
            }
        }
        Node::ParenthesesNode { .. } => {
            // (begin ...): every statement is a child.
            let parens = statement.as_parentheses_node();
            let statements = parens.and_then(|p| p.body()).and_then(|b| b.as_statements_node());
            if let Some(statements) = statements {
                found.extend(
                    statements.body().iter().filter_map(|s| Node::as_call_node(&s)).filter(is_send),
                );
            }
        }
        Node::BeginNode { .. } => {
            // `kwbegin`: its statements are children, unless it has `rescue`/`ensure`.
            if let Some(begin) = statement.as_begin_node() {
                if begin.rescue_clause().is_none() && begin.ensure_clause().is_none() {
                    if let Some(statements) = begin.statements() {
                        found.extend(
                            statements
                                .body()
                                .iter()
                                .filter_map(|s| Node::as_call_node(&s))
                                .filter(is_send),
                        );
                    }
                }
            }
        }
        _ => {
            let mut push = |child: &Node<'pr>| {
                if let Some(call) = child.as_call_node().filter(is_send) {
                    found.push(call);
                }
            };
            for_each_child(statement, |child| match child {
                // A single-statement body is elided to the statement.
                Node::StatementsNode { .. } => {
                    if let Some(only) = only_statement(child.as_statements_node()) {
                        push(&only);
                    }
                }
                Node::ArgumentsNode { .. } => {
                    if let Some(arguments) = child.as_arguments_node() {
                        arguments.arguments().iter().for_each(|a| push(&a));
                    }
                }
                _ => push(child),
            });
        }
    }
    found
}

/// `assertion_method?(send_node)`.
fn assertion_method(call: &CallNode<'_>) -> bool {
    let name = call.name();
    let name = name.as_slice();
    // assertion_prefix_method?
    (call.receiver().is_none() && (name.starts_with(b"assert") || name.starts_with(b"refute")))
        || VALUE_MATCHERS.contains(&name)
        || BLOCK_MATCHERS.contains(&name)
}

fn assertion_span(call: &CallNode<'_>) -> Span {
    if call.block().is_some_and(|b| b.as_block_node().is_some()) {
        call_span_excluding_block(call)
    } else {
        call.as_node().span()
    }
}

/// `assertion.each_descendant(:gvar).any? { |d| d.children.first == gvar_name }`,
/// over the `send` part of the assertion only.
fn references_gvar(call: &CallNode<'_>, gvar_name: &[u8]) -> bool {
    fn descend(node: &Node<'_>, gvar_name: &[u8]) -> bool {
        let mut found = false;
        for_each_child(node, |child| {
            if found || child.as_block_node().is_some() {
                return;
            }
            if let Some(read) = child.as_global_variable_read_node() {
                if read.name().as_slice() == gvar_name {
                    found = true;
                    return;
                }
            }
            found = descend(child, gvar_name);
        });
        found
    }
    descend(&call.as_node(), gvar_name)
}
