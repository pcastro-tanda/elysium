//! `Lint/UnreachableCode`, ported from RuboCop's
//! `lib/rubocop/cop/lint/unreachable_code.rb`.
//!
//! # `StatementsNode` unifies `on_begin`/`on_kwbegin`
//!
//! Whitequark only wraps a statement sequence in a `:begin`/`:kwbegin` node
//! when it holds more than one statement (a single-statement method/block/
//! branch body is that statement directly), so upstream's `on_begin` (which
//! `each_cons(2)`-scans a `begin`/`kwbegin`'s children for a
//! flow-terminating statement followed by another) only ever fires where
//! there is more than one statement to scan. Prism instead wraps *every*
//! statement sequence -- a method body, a block body, an `if`/`case`
//! branch, the top-level program, an explicit `begin...end` -- in a
//! [`ruby_ast::NodeKind::StatementsNode`], one or more statements alike (see
//! `Lint/ConstantDefinitionInBlock`'s module doc for the same observation).
//! Subscribing to that single kind and windowing its `body` by twos
//! therefore reproduces `on_begin`/`on_kwbegin` for every one of those
//! shapes uniformly, with no special-casing needed for "one versus several
//! statements".
//!
//! # `flow_expression?` recursion
//!
//! [`UnreachableCode::flow_expression`] is a direct, non-reporting port of
//! upstream's private `flow_expression?`: given a single statement, does it
//! unconditionally divert control flow (a bare `return`/`next`/`break`/
//! `redo`/`retry`, a `raise`/`fail`/`throw`/`exit`/`exit!`/`abort` `send`
//! (never `csend`/safe-navigated) call with no receiver or an explicit
//! `Kernel`/`::Kernel` receiver, or an `if`/
//! `unless`/`case`/`case`-`in` whose every branch does)? Its recursion
//! only ever descends through other pure statement-sequencing constructs
//! (`begin`/`if`/`unless`/`case`/`case`-`in`), never through a block or
//! `def`, so it never revisits a subtree the engine's own traversal will
//! reach independently (mirroring `Style/RedundantReturn`'s equivalent
//! non-revisiting `check_branch` walk) -- except that a nested *explicit*
//! `begin...end`/implicit multi-statement grouping *is* also a
//! `StatementsNode`-bearing node the engine will separately re-enter and
//! `each_cons`-scan on its own account, exactly like upstream's `on_begin`
//! independently firing for that same nested node; any `def`/`defs`
//! redefinition side effect this recursion triggers along the way (see
//! below) is therefore sometimes registered twice for the same statement,
//! which is harmless since `@redefined`/[`UnreachableCode::redefined`] is
//! an append-only list checked only by membership, exactly like upstream's
//! `Array#<<`/`Array#include?`.
//!
//! A `begin`/`kwbegin` in this recursion is Prism's [`BeginNode`] *without*
//! a `rescue`/`else`/`ensure` clause (with one, it is not a `:begin`/
//! `:kwbegin` node type upstream at all, but a `:rescue`/`:ensure` one,
//! which `flow_expression?`'s `case` has no branch for and so always
//! answers `false` on; see `shadowed_exception.rs`'s module doc for the
//! `BeginNode`-without-keyword proof).
//!
//! `unless` is upstream's same `:if` node type (whitequark normalizes its
//! swapped branches into the same `if_branch`/`else_branch` shape
//! `check_if` reads), reached here as Prism's own distinct
//! [`ruby_ast::node::UnlessNode`] kind through
//! [`UnreachableCode::check_unless`] instead -- it has no `elsif`-equivalent
//! chain to recurse through, unlike [`UnreachableCode::check_if`].
//!
//! An `if`/`unless` branch or `when`/`in`/`case`-`else` branch with no
//! statements at all (`Option::None`, never an empty `StatementsNode` --
//! Prism omits the node entirely rather than emit an empty one) is treated
//! as absent, matching upstream's `node_parts` normalizing a truly empty
//! branch to `nil` (indistinguishable, on both sides of this port, from
//! "no such branch").
//!
//! # `@redefined`/`@instance_eval_count` ordering
//!
//! Both are upstream's mutable, whole-file-lifetime bookkeeping, populated
//! and consulted purely in the traversal's document order -- exactly the
//! order the engine's own pre-order `enter` calls arrive in, so
//! [`UnreachableCode::redefined`] and [`UnreachableCode::instance_eval_depth`]
//! are simply `&mut self` fields threaded through, reset once per file in
//! [`Rule::file_start`].
//!
//! `report_on_flow_command`'s `@instance_eval_count.positive?` check applies
//! only to a bare bare-receiver `raise`/`fail`/`throw`/`exit`/`exit!`/
//! `abort` call reached via [`UnreachableCode::flow_expression`]'s
//! recursion -- and that recursion only ever descends through `begin`/`if`/
//! `unless`/`case`/`case`-`in` wrappers, never through a block, so any such
//! call it reaches sits at *exactly* the same `instance_eval`-block nesting
//! depth as the [`ruby_ast::NodeKind::StatementsNode`] the recursion was
//! entered from. `instance_eval_depth` can therefore be read directly off
//! `self` at the point `enter(StatementsNode)` fires (real engine-driven
//! traversal position) rather than needing a per-node ancestor walk;
//! [`ruby_ast::NodeKind::CallNode`]'s own `enter`/`leave` push/pop it
//! whenever the call is an `instance_eval` with a literal block attached
//! (any receiver, matching upstream's receiver-agnostic
//! `node.method?(:instance_eval)`), covering a plain block, a numbered-
//! parameter block, and an `it`-block alike, since Prism represents all
//! three with the one `BlockNode` kind (matching upstream's
//! `on_block`/`on_numblock`/`on_itblock` aliases).
//!
//! `redefinable_flow_method?`'s method list is the same six names
//! [`FLOW_METHODS`] restricts `flow_command?`'s call shape to; a `def`/`defs`
//! for any other method name is visited by
//! [`UnreachableCode::register_redefinition`] (matching upstream's
//! `register_redefinition` being called unconditionally for every `def`/
//! `defs` the recursion reaches) but never actually recorded.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::{CallNode, CaseMatchNode, CaseNode, IfNode, StatementsNode, UnlessNode};
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Unreachable code detected.";

/// RuboCop's `redefinable_flow_method?` list, shared with `flow_command?`'s
/// call-shape restriction.
const FLOW_METHODS: &[&[u8]] = &[b"raise", b"fail", b"throw", b"exit", b"exit!", b"abort"];

/// Checks for unreachable code.
///
/// The check are based on the presence of flow of control
/// statement in non-final position in `begin` (implicit) blocks.
///
/// ```ruby
/// # bad
/// def some_method
///   return
///   do_something
/// end
///
/// # bad
/// def some_method
///   if cond
///     return
///   else
///     return
///   end
///   do_something
/// end
///
/// # good
/// def some_method
///   do_something
/// end
/// ```
#[derive(Debug, Clone, Default)]
pub struct UnreachableCode {
    /// RuboCop's `@redefined`: bare method names among [`FLOW_METHODS`]
    /// redefined by a `def`/`defs` statement the `flow_expression?`
    /// recursion has reached so far, in file order. An append-only list
    /// (matching upstream's `Array#<<`), checked only by membership.
    redefined: Vec<Vec<u8>>,
    /// RuboCop's `@instance_eval_count`: nesting depth of
    /// `X.instance_eval do ... end`-shaped blocks at the engine's current
    /// traversal position.
    instance_eval_depth: u32,
}

impl UnreachableCode {
    /// RuboCop's `instance_eval_block?`, applied at `on_block`/`after_block`
    /// time: any block (plain, numbered-parameter, or `it`) whose owning
    /// call is named `instance_eval`, any receiver.
    fn is_instance_eval_block(call: &CallNode<'_>) -> bool {
        call.name().as_slice() == b"instance_eval"
            && call.block().is_some_and(|b| b.as_block_node().is_some())
    }

    /// RuboCop's `flow_command?`'s `send` alternative, restricted to the
    /// call shape: one of [`FLOW_METHODS`], with no receiver or an explicit
    /// bare/top-level-qualified `Kernel` receiver.
    fn is_flow_command_call(call: &CallNode<'_>) -> bool {
        if call.is_safe_navigation() {
            return false;
        }
        if !FLOW_METHODS.contains(&call.name().as_slice()) {
            return false;
        }
        match call.receiver() {
            None => true,
            Some(receiver) => {
                ext::is_bare_or_toplevel_const(&receiver)
                    && ext::const_name(&receiver).as_deref() == Some("Kernel")
            }
        }
    }

    /// RuboCop's `report_on_flow_command?`, applied to a `CallNode` already
    /// known to match [`Self::is_flow_command_call`] (a keyword flow
    /// command never reaches this: `flow_expression` returns `true` for one
    /// directly, matching `report_on_flow_command?`'s own `return true
    /// unless node.send_type?`).
    fn report_on_flow_command(&self, call: &CallNode<'_>) -> bool {
        // By the contract of `flow_command?`, a receiver here means the
        // call is on `Kernel`, always reported.
        if call.receiver().is_some() {
            return true;
        }
        // Inside `instance_eval` there is no way to tell `self`'s type
        // from the AST alone, so a redefinition can never be ruled out;
        // silence the warning.
        if self.instance_eval_depth > 0 {
            return false;
        }
        !self.redefined.iter().any(|name| name.as_slice() == call.name().as_slice())
    }

    /// RuboCop's `register_redefinition`.
    fn register_redefinition(&mut self, name: &[u8]) {
        if FLOW_METHODS.contains(&name) {
            self.redefined.push(name.to_vec());
        }
    }

    /// RuboCop's `flow_expression?`'s recursive `expressions.any? { |expr|
    /// flow_expression?(expr) }`/branch-body check, applied to a normalized
    /// branch body: `None` (an empty or absent branch) is never
    /// flow-terminating; `Some` recurses into every statement, mirroring
    /// that a single-statement Prism `StatementsNode` behaves exactly like
    /// a multi-statement one under `.any?`.
    fn statements_any(&mut self, stmts: Option<StatementsNode<'_>>) -> bool {
        let Some(stmts) = stmts else { return false };
        for statement in &stmts.body() {
            if self.flow_expression(&statement) {
                return true;
            }
        }
        false
    }

    /// RuboCop's `flow_expression?`.
    fn flow_expression(&mut self, node: &Node<'_>) -> bool {
        match node.kind() {
            NodeKind::ReturnNode
            | NodeKind::NextNode
            | NodeKind::BreakNode
            | NodeKind::RedoNode
            | NodeKind::RetryNode => true,
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                if !Self::is_flow_command_call(&call) {
                    return false;
                }
                self.report_on_flow_command(&call)
            }
            NodeKind::BeginNode => {
                let begin = node.as_begin_node().expect("kind matched");
                if begin.rescue_clause().is_some()
                    || begin.else_clause().is_some()
                    || begin.ensure_clause().is_some()
                {
                    // Not a `:begin`/`:kwbegin` node upstream at all.
                    return false;
                }
                self.statements_any(begin.statements())
            }
            NodeKind::IfNode => self.check_if(&node.as_if_node().expect("kind matched")),
            NodeKind::UnlessNode => {
                self.check_unless(&node.as_unless_node().expect("kind matched"))
            }
            NodeKind::CaseNode => self.check_case(&node.as_case_node().expect("kind matched")),
            NodeKind::CaseMatchNode => {
                self.check_case_match(&node.as_case_match_node().expect("kind matched"))
            }
            NodeKind::DefNode => {
                let def = node.as_def_node().expect("kind matched");
                self.register_redefinition(def.name().as_slice());
                false
            }
            _ => false,
        }
    }

    /// RuboCop's `check_if`: `if_branch && else_branch &&
    /// flow_expression?(if_branch) && flow_expression?(else_branch)`, with
    /// `else_branch` normalized from `subsequent` (an `elsif` -- another
    /// `IfNode`, recursed into directly, matching whitequark's nested-`if`
    /// normalization -- or a real `ElseNode`'s own statements).
    fn check_if(&mut self, node: &IfNode<'_>) -> bool {
        if node.statements().is_none() {
            return false;
        }
        let Some(subsequent) = node.subsequent() else { return false };
        let else_present = match &subsequent {
            Node::IfNode { .. } => true,
            Node::ElseNode { .. } => {
                subsequent.as_else_node().expect("kind matched").statements().is_some()
            }
            _ => unreachable!("IfNode#subsequent is always an if or an else"),
        };
        if !else_present {
            return false;
        }
        if !self.statements_any(node.statements()) {
            return false;
        }
        match &subsequent {
            Node::IfNode { .. } => self.flow_expression(&subsequent),
            Node::ElseNode { .. } => {
                self.statements_any(subsequent.as_else_node().expect("kind matched").statements())
            }
            _ => unreachable!("IfNode#subsequent is always an if or an else"),
        }
    }

    /// RuboCop's `check_if` for `unless`, which has no `elsif`-equivalent
    /// chain.
    fn check_unless(&mut self, node: &UnlessNode<'_>) -> bool {
        if node.statements().is_none() {
            return false;
        }
        let Some(else_node) = node.else_clause() else { return false };
        if else_node.statements().is_none() {
            return false;
        }
        if !self.statements_any(node.statements()) {
            return false;
        }
        self.statements_any(else_node.statements())
    }

    /// RuboCop's `check_case`, for a plain `case`/`when`.
    fn check_case(&mut self, node: &CaseNode<'_>) -> bool {
        let Some(else_node) = node.else_clause() else { return false };
        if !self.statements_any(else_node.statements()) {
            return false;
        }
        for branch in &node.conditions() {
            let when = branch.as_when_node().expect("case conditions are when nodes");
            if !self.statements_any(when.statements()) {
                return false;
            }
        }
        true
    }

    /// RuboCop's `check_case`, for `case`/`in` pattern matching.
    fn check_case_match(&mut self, node: &CaseMatchNode<'_>) -> bool {
        let Some(else_node) = node.else_clause() else { return false };
        if !self.statements_any(else_node.statements()) {
            return false;
        }
        for branch in &node.conditions() {
            let in_pattern = branch.as_in_node().expect("case/in conditions are in nodes");
            if !self.statements_any(in_pattern.statements()) {
                return false;
            }
        }
        true
    }
}

impl Rule for UnreachableCode {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UnreachableCode",
        department: Department::Lint,
        summary: "Checks for unreachable code.",
        explanation: "\
The check are based on the presence of flow of control
statement in non-final position in `begin` (implicit) blocks.

```ruby
# bad
def some_method
  return
  do_something
end

# bad
def some_method
  if cond
    return
  else
    return
  end
  do_something
end

# good
def some_method
  do_something
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "\
`retry` outside a `rescue`/`begin` clause -- upstream's own fixture for it
is skipped under a Prism-backed parser, since Prism (unlike `parser`) treats
a top-level `retry` as a syntax error rather than a valid (if pointless)
keyword, so no such source can ever reach this rule to exercise the branch.
`@redefined`/`@instance_eval_count`-based suppression is otherwise exact:
a `def`/`def self.` for one of `raise`/`fail`/`throw`/`exit`/`exit!`/
`abort` only counts when it sits somewhere the `flow_expression?` recursion
actually reaches (a statement checked directly, or nested in a `begin`/
`if`/`unless`/`case`/`case`-`in` branch it recurses through) -- a
redefinition nested inside another `def`'s own body, a block, or a class/
module body is never seen, matching upstream's identical blind spot.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.redefined.clear();
        self.instance_eval_depth = 0;
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                if Self::is_instance_eval_block(&call) {
                    self.instance_eval_depth += 1;
                }
            }
            NodeKind::StatementsNode => {
                let stmts = node.as_statements_node().expect("kind matched");
                let body = stmts.body();
                if body.len() < 2 {
                    return;
                }
                let mut prev: Option<Node<'_>> = None;
                for statement in &body {
                    if let Some(prev_stmt) = prev.take() {
                        if self.flow_expression(&prev_stmt) {
                            ctx.report(&Self::META, statement.span(), MSG);
                        }
                    }
                    prev = Some(statement);
                }
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.kind() == NodeKind::CallNode {
            let call = node.as_call_node().expect("kind matched");
            if Self::is_instance_eval_block(&call) {
                self.instance_eval_depth -= 1;
            }
        }
    }
}
