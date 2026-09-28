//! `Lint/UnreachableLoop`, ported from RuboCop's
//! `lib/rubocop/cop/lint/unreachable_loop.rb` plus the `AllowedPattern`
//! mixin it includes.
//!
//! # Node shapes
//!
//! `while`/`until`/`for` are their own Prism kinds ([`NodeKind::WhileNode`],
//! [`NodeKind::UntilNode`], [`NodeKind::ForNode`]) whose own span already
//! covers the entire `keyword ... end` construct, matching whitequark's
//! node for the same construct (confirmed empirically -- see the type doc
//! on [`UnreachableLoop::check`]). A loop-like block (`.each { }`, `loop
//! do...end`, ...) is different: whitequark's parser wraps the receiver,
//! method dispatch, and block into one `(block (send ...) args body)` node
//! and hands *that* to `on_block`, so `add_offense(node)` highlights the
//! whole chain -- receiver through closing brace. Prism instead attaches
//! the block as a field on the owning [`NodeKind::CallNode`]
//! (`CallNode::block`), and that outer call's own span already extends
//! through the block's closing delimiter (confirmed empirically), so this
//! port reports at the *owning call's* span rather than the
//! [`NodeKind::BlockNode`]'s own (which only covers `{ ... }`/`do ... end`).
//! Since [`Context::ancestors`] only carries `(kind, span)` and this rule
//! needs the owning `CallNode`'s method name to classify `loop_method?` and
//! its pre-block source text for `AllowedPatterns`, [`UnreachableLoop`]
//! keeps its own stack of "is this call, if it owns a block, a loop
//! method" facts, pushed on entering a block-owning `CallNode` and popped
//! on leaving it (see `constant_definition_in_block.rs`/`missing_super.rs`
//! for the same pattern applied to other ancestor data `Context` cannot
//! carry).
//!
//! # `statements`/`break_statement?`
//!
//! Whitequark elides the wrapping node for a body holding exactly one
//! statement (the body IS that statement) and only wraps multiple
//! statements in a `begin` node -- upstream's `statements` helper undoes
//! exactly that distinction, replacing a `begin`-type body with its
//! flattened children and leaving anything else as a single-element list.
//! Prism has no such elision: every non-empty body (a loop's, an
//! `if`/`when`/`in`/`else` branch's, or a keyword `begin...end` block's) is
//! always a [`NodeKind::StatementsNode`], holding one child or several
//! alike -- so [`Body::from_statements`] just reads that list directly,
//! with no `begin_type?`-style special case needed. This also means an
//! *explicit* `begin...end` appearing as one statement among several (or
//! as a post-condition loop's entire body, Ruby requiring a `kwbegin`
//! there) is its own nested [`NodeKind::BeginNode`], handled by
//! [`UnreachableLoop::is_break_statement`]'s recursive case exactly as
//! upstream's `:begin`/`:kwbegin` branch of `break_statement?` handles it
//! (confirmed empirically that a post-condition while's `statements()`
//! holds exactly one child, a `BeginNode`, mirroring whitequark's `else
//! [body]` fallback for a `kwbegin`-typed body that never matches its
//! `begin_type?` check) -- *unless* that `BeginNode` carries a
//! `rescue`/`else`/`ensure` clause, in which case whitequark types it a
//! `:rescue`/`:ensure` node instead (never `:begin`/`:kwbegin`), which
//! `break_statement?`'s `case` has no branch for and so always answers
//! `false` on; [`UnreachableLoop::is_break_statement`] checks for those
//! clauses before recursing, mirroring `Lint/UnreachableCode`'s identical
//! `BeginNode` guard. The same shape recurs in [`block_body`]'s `Single`
//! case: a block whose sole implicit statement is a bare `do ... rescue
//! ... end` is, again, a `BeginNode` with a `rescue_clause`, so it is
//! likewise never treated as a break statement.
//!
//! [`UnreachableLoop::branch_breaks`] (upstream's shared logic between
//! `check`'s own `statements.find`/`preceded_by_continue_statement?` pair
//! and the recursive `:begin`/`:kwbegin` case) doubles as the "does this
//! `if`/`when`/`in`/`else` branch break" check besides: a Prism branch body
//! holding a single statement is *itself* a one-element `StatementsNode`,
//! so scanning it the same way as a multi-statement body/loop body
//! degenerates to exactly upstream's un-wrapped single-node case (no
//! preceding sibling ever exists to misclassify as a "preceding continue").
//!
//! `if_branch && else_branch && break_statement?(if_branch) &&
//! break_statement?(else_branch)` recurses on an `elsif` chain for free:
//! upstream's `else_branch` for an `if...elsif...else...end` returns the
//! nested `elsif` `if`-node itself (whitequark represents `elsif` as a
//! nested `if` in the `else` slot), so `break_statement?` dispatches back
//! into `check_if` on it. Prism's `IfNode::subsequent` is the same node
//! either way (a nested `IfNode` for `elsif`, an `ElseNode` for a final
//! `else`), so [`UnreachableLoop::check_if`] mirrors that dispatch
//! directly.
//!
//! # `conditional_continue_keyword?`
//!
//! Only applied once, to the single top-level statement `check` already
//! chose as the loop's "does it break" witness -- never re-applied inside
//! nested `if`/`case`/`begin` recursion, matching upstream (only `check`'s
//! own call site guards with it; the `:begin`/`:kwbegin` recursive case
//! only re-checks `preceded_by_continue_statement?`). `CONTINUE_KEYWORDS`
//! is `next`/`redo` only -- `return foo || break` is *not* exempted, since
//! `:break` is never in that list.

use linter::{ConfigDefault, ConfigOption};
use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use regex::Regex;
use ruby_ast::node::{CallNode, CaseMatchNode, CaseNode, IfNode, NodeListIter, StatementsNode};
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "This loop will have at most one iteration.";

/// RuboCop-AST's `MethodIdentifierPredicates::ENUMERATOR_METHODS`.
const ENUMERATOR_METHODS: &[&[u8]] = &[
    b"collect",
    b"collect_concat",
    b"detect",
    b"downto",
    b"each",
    b"find",
    b"find_all",
    b"find_index",
    b"inject",
    b"loop",
    b"map!",
    b"map",
    b"reduce",
    b"reject",
    b"reject!",
    b"reverse_each",
    b"select",
    b"select!",
    b"times",
    b"upto",
];

/// RuboCop-AST's `MethodIdentifierPredicates::ENUMERABLE_METHODS`
/// (`Enumerable.instance_methods + [:each]`).
const ENUMERABLE_METHODS: &[&[u8]] = &[
    b"all?",
    b"any?",
    b"chain",
    b"chunk",
    b"chunk_while",
    b"collect",
    b"collect_concat",
    b"compact",
    b"count",
    b"cycle",
    b"detect",
    b"drop",
    b"drop_while",
    b"each",
    b"each_cons",
    b"each_entry",
    b"each_slice",
    b"each_with_index",
    b"each_with_object",
    b"entries",
    b"filter",
    b"filter_map",
    b"find",
    b"find_all",
    b"find_index",
    b"first",
    b"flat_map",
    b"grep",
    b"grep_v",
    b"group_by",
    b"include?",
    b"inject",
    b"lazy",
    b"map",
    b"max",
    b"max_by",
    b"member?",
    b"min",
    b"min_by",
    b"minmax",
    b"minmax_by",
    b"none?",
    b"one?",
    b"partition",
    b"reduce",
    b"reject",
    b"reverse_each",
    b"select",
    b"slice_after",
    b"slice_before",
    b"slice_when",
    b"sort",
    b"sort_by",
    b"sum",
    b"take",
    b"take_while",
    b"tally",
    b"to_a",
    b"to_h",
    b"to_set",
    b"uniq",
    b"zip",
];

/// RuboCop-AST's `enumerator_method?`/`enumerable_method?`, combined (both
/// feed `loop_method?`'s `||` chain alongside `method?(:loop)`, already
/// covered here since `:loop` is itself one of the [`ENUMERATOR_METHODS`]).
fn is_loopable_method_name(name: &[u8]) -> bool {
    ENUMERATOR_METHODS.contains(&name)
        || name.starts_with(b"each_")
        || ENUMERABLE_METHODS.contains(&name)
}

/// RuboCop's `CONTINUE_KEYWORDS`.
fn is_continue_kind(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::NextNode | NodeKind::RedoNode)
}

/// A loop's, branch's, or block's body, abstracted over the two shapes
/// [`UnreachableLoop`] needs to scan: the common case of a
/// [`StatementsNode`]'s children (a loop's/branch's own body -- see the
/// module doc for why Prism needs no `begin_type?`-style unwrapping here),
/// or a lone non-`StatementsNode` node ([`BlockNode`]'s own body, when it
/// holds exactly one statement that is itself e.g. a `BeginNode` with
/// rescue clauses attached -- RuboCop-AST's analogous "body, or single
/// implicit statement" distinction the other node kinds don't need one
/// of, since Prism never wraps a block's *own* body in an extra layer).
/// `Copy`, so scanning it twice (once to find a break statement, once for
/// the statements preceding it) just re-derives a fresh, pointer-cheap
/// iterator each time rather than collecting a `Vec` up front.
#[derive(Clone, Copy)]
enum Body<'pr> {
    Empty,
    Stmts(StatementsNode<'pr>),
    Single(Node<'pr>),
}

impl<'pr> Body<'pr> {
    /// `stmts.body()`'s children, or none if the body is empty.
    fn from_statements(stmts: Option<StatementsNode<'pr>>) -> Self {
        stmts.map_or(Self::Empty, Self::Stmts)
    }

    fn iter(self) -> BodyIter<'pr> {
        match self {
            Self::Empty => BodyIter::Empty,
            Self::Stmts(s) => BodyIter::Stmts(s.body().iter()),
            Self::Single(n) => BodyIter::Single(std::iter::once(n)),
        }
    }
}

/// [`Body::iter`]'s iterator: a real [`NodeListIter`] for the common case,
/// or a zero-/one-element iterator for the rest -- no allocation either
/// way.
enum BodyIter<'pr> {
    Empty,
    Stmts(NodeListIter<'pr>),
    Single(std::iter::Once<Node<'pr>>),
}

impl<'pr> Iterator for BodyIter<'pr> {
    type Item = Node<'pr>;

    fn next(&mut self) -> Option<Node<'pr>> {
        match self {
            Self::Empty => None,
            Self::Stmts(it) => it.next(),
            Self::Single(it) => it.next(),
        }
    }
}

/// `BlockNode::body`'s "body, or single implicit statement" shape -- see
/// [`Body`]'s doc.
fn block_body(body: Option<Node<'_>>) -> Body<'_> {
    match body {
        None => Body::Empty,
        Some(Node::StatementsNode { .. }) => {
            Body::from_statements(body.and_then(|b| b.as_statements_node()))
        }
        Some(other) => Body::Single(other),
    }
}

/// Checks for loops that will have at most one iteration.
///
/// A loop that can never reach the second iteration is a possible error in
/// the code. In rare cases where only one iteration (or at most one
/// iteration) is intended behavior, the code should be refactored to use
/// `if` conditionals.
///
/// NOTE: Block methods that are used with `Enumerable`s are considered to
/// be loops.
///
/// `AllowedPatterns` can be used to match against the block receiver in
/// order to allow code that would otherwise be registered as an offense
/// (eg. `times` used not in an `Enumerable` context).
///
/// # Examples
///
/// ```ruby
/// # bad
/// while node
///   do_something(node)
///   node = node.parent
///   break
/// end
///
/// # good
/// while node
///   do_something(node)
///   node = node.parent
/// end
///
/// # bad
/// def verify_list(head)
///   item = head
///   begin
///     if verify(item)
///       return true
///     else
///       return false
///     end
///   end while(item)
/// end
///
/// # good
/// def verify_list(head)
///   item = head
///   begin
///     if verify(item)
///       item = item.next
///     else
///       return false
///     end
///   end while(item)
///
///   true
/// end
///
/// # bad
/// def find_something(items)
///   items.each do |item|
///     if something?(item)
///       return item
///     else
///       raise NotFoundError
///     end
///   end
/// end
///
/// # good
/// def find_something(items)
///   items.each do |item|
///     if something?(item)
///       return item
///     end
///   end
///   raise NotFoundError
/// end
///
/// # bad
/// 2.times { raise ArgumentError }
/// ```
///
/// With `AllowedPatterns: ['(exactly|at_least|at_most)\(\d+\)\.times']` (the default):
///
/// ```ruby
/// # good
/// exactly(2).times { raise StandardError }
/// ```
#[derive(Debug, Clone)]
pub struct UnreachableLoop {
    /// `AllowedPatterns`.
    allowed_patterns: Vec<Regex>,
    /// Facts about the nearest enclosing block-owning `CallNode`s, pushed
    /// on entering such a call and popped on leaving it. `Some(span)` when
    /// that call is itself a loop method (RuboCop's `loop_method?`),
    /// carrying the call's own full span (through the block's closing
    /// delimiter) to report at; `None` when it is not. See the module doc.
    block_call_stack: Vec<Option<Span>>,
}

impl UnreachableLoop {
    /// RuboCop's `loop_method?`, applied directly to the call node owning a
    /// block (rather than to the whitequark block node itself, which has
    /// no Prism equivalent -- see the module doc).
    fn is_loop_method_call(&self, call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
        let name = call.name();
        if !is_loopable_method_name(name.as_slice()) {
            return false;
        }
        let source = String::from_utf8_lossy(ctx.text(ext::call_span_excluding_block(call)));
        !self.allowed_patterns.iter().any(|pattern| pattern.is_match(&source))
    }

    /// RuboCop's `preceded_by_continue_statement?`, applied to `sibling` --
    /// one candidate node (upstream's `Node#loop_keyword?` covers a
    /// `while`/`until`/`for`; `loop_method?` needs the call node owning a
    /// block, present here only when `sibling` is itself such a call).
    fn is_loop_sibling(&self, sibling: &Node<'_>, ctx: &Context<'_>) -> bool {
        match sibling.kind() {
            NodeKind::WhileNode | NodeKind::UntilNode | NodeKind::ForNode => true,
            NodeKind::CallNode => {
                let call = sibling.as_call_node().expect("kind matched");
                matches!(call.block(), Some(Node::BlockNode { .. }))
                    && self.is_loop_method_call(&call, ctx)
            }
            _ => false,
        }
    }

    /// RuboCop's `preceded_by_continue_statement?`.
    fn preceded_by_continue<'pr>(
        &self,
        siblings: impl Iterator<Item = Node<'pr>>,
        ctx: &Context<'_>,
    ) -> bool {
        siblings
            .filter(|sibling| !self.is_loop_sibling(sibling, ctx))
            .any(|sibling| has_continue_descendant(&sibling))
    }

    /// The shared "does this statement list unconditionally break" check:
    /// upstream's `check`'s own `statements.find`/
    /// `preceded_by_continue_statement?` pair, reused for the `:begin`/
    /// `:kwbegin` recursive case in `break_statement?` and for each
    /// `if`/`when`/`in`/`else` branch body (see the module doc for why a
    /// single-statement Prism branch degenerates to the same un-wrapped
    /// case upstream special-cases).
    fn branch_breaks(&self, body: Body<'_>, ctx: &Context<'_>) -> bool {
        let Some(idx) = body.iter().position(|child| self.is_break_statement(&child, ctx)) else {
            return false;
        };
        !self.preceded_by_continue(body.iter().take(idx), ctx)
    }

    /// RuboCop's `break_statement?`.
    fn is_break_statement(&self, node: &Node<'_>, ctx: &Context<'_>) -> bool {
        if is_break_command(node) {
            return true;
        }
        match node.kind() {
            NodeKind::BeginNode => {
                let begin = node.as_begin_node().expect("kind matched");
                if begin.rescue_clause().is_some()
                    || begin.else_clause().is_some()
                    || begin.ensure_clause().is_some()
                {
                    // Not a `:begin`/`:kwbegin` node upstream at all --
                    // whitequark types this a `:rescue`/`:ensure` node,
                    // which `break_statement?`'s `case` has no branch for.
                    false
                } else {
                    self.branch_breaks(Body::from_statements(begin.statements()), ctx)
                }
            }
            NodeKind::IfNode => self.check_if(&node.as_if_node().expect("kind matched"), ctx),
            NodeKind::CaseNode => self.check_case(&node.as_case_node().expect("kind matched"), ctx),
            NodeKind::CaseMatchNode => {
                self.check_case_match(&node.as_case_match_node().expect("kind matched"), ctx)
            }
            _ => false,
        }
    }

    /// RuboCop's `check_if`.
    fn check_if(&self, node: &IfNode<'_>, ctx: &Context<'_>) -> bool {
        if !self.branch_breaks(Body::from_statements(node.statements()), ctx) {
            return false;
        }
        match node.subsequent() {
            Some(Node::IfNode { .. }) => self
                .check_if(&node.subsequent().and_then(|n| n.as_if_node()).expect("matched"), ctx),
            Some(Node::ElseNode { .. }) => {
                let else_node = node.subsequent().and_then(|n| n.as_else_node()).expect("matched");
                self.branch_breaks(Body::from_statements(else_node.statements()), ctx)
            }
            _ => false,
        }
    }

    /// RuboCop's `check_case`, applied to a `case`/`when` node.
    fn check_case(&self, node: &CaseNode<'_>, ctx: &Context<'_>) -> bool {
        let Some(else_node) = node.else_clause() else { return false };
        if !self.branch_breaks(Body::from_statements(else_node.statements()), ctx) {
            return false;
        }
        node.conditions().iter().all(|cond| {
            let when = cond.as_when_node().expect("case conditions are WhenNode");
            self.branch_breaks(Body::from_statements(when.statements()), ctx)
        })
    }

    /// RuboCop's `check_case`, applied to a `case`/`in` node.
    fn check_case_match(&self, node: &CaseMatchNode<'_>, ctx: &Context<'_>) -> bool {
        let Some(else_node) = node.else_clause() else { return false };
        if !self.branch_breaks(Body::from_statements(else_node.statements()), ctx) {
            return false;
        }
        node.conditions().iter().all(|cond| {
            let in_pattern = cond.as_in_node().expect("case/in conditions are InNode");
            self.branch_breaks(Body::from_statements(in_pattern.statements()), ctx)
        })
    }

    /// RuboCop's `check`: `node` is the loop itself (a `while`/`until`/`for`,
    /// whose own span already covers the whole `keyword ... end`
    /// construct) or, for a loop-like block, the *owning call*'s span (see
    /// the module doc for why that differs from the `BlockNode`'s own).
    fn check(&self, body: Body<'_>, report_span: Span, ctx: &mut Context<'_>) {
        let Some(idx) = body.iter().position(|child| self.is_break_statement(&child, ctx)) else {
            return;
        };
        let break_statement = body.iter().nth(idx).expect("idx came from this same body");
        if self.preceded_by_continue(body.iter().take(idx), ctx)
            || conditional_continue_keyword(&break_statement)
        {
            return;
        }
        ctx.report(&Self::META, report_span, MSG);
    }
}

/// RuboCop's `break_command?` node matcher: `return`/`break`, or a call to
/// `raise`/`fail`/`throw`/`exit`/`exit!`/`abort` with no receiver or a bare
/// (or top-level-qualified) `Kernel` receiver -- never a safe-navigation
/// call, since the upstream pattern only ever matches a plain `send`.
fn is_break_command(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::ReturnNode | NodeKind::BreakNode => true,
        NodeKind::CallNode => {
            let call = node.as_call_node().expect("kind matched");
            if call.is_safe_navigation() {
                return false;
            }
            let name = call.name();
            if !matches!(
                name.as_slice(),
                b"raise" | b"fail" | b"throw" | b"exit" | b"exit!" | b"abort"
            ) {
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
        _ => false,
    }
}

/// Pre-order search for a descendant matching `pred`: unlike
/// [`ruby_ast::each_descendant`] (which unconditionally visits every node
/// in the subtree), this returns as soon as a match is found, skipping the
/// rest of the subtree at every level still left to visit.
fn any_descendant<'pr>(node: &Node<'pr>, pred: &impl Fn(&Node<'pr>) -> bool) -> bool {
    let mut found = false;
    ruby_ast::for_each_child(node, |child| {
        if !found {
            found = pred(child) || any_descendant(child, pred);
        }
    });
    found
}

/// RuboCop's `sibling.each_descendant(*CONTINUE_KEYWORDS).any?`.
fn has_continue_descendant(node: &Node<'_>) -> bool {
    any_descendant(node, &|child| is_continue_kind(child.kind()))
}

/// RuboCop's `conditional_continue_keyword?`: the last `or` node anywhere
/// among `node`'s descendants has a `next`/`redo` right-hand side (`break`
/// is not a `CONTINUE_KEYWORDS` member, so `return foo || break` is never
/// exempted this way).
fn conditional_continue_keyword(node: &Node<'_>) -> bool {
    let mut last_or_rhs: Option<NodeKind> = None;
    ruby_ast::each_descendant(node, &mut |child| {
        if let Node::OrNode { .. } = child {
            last_or_rhs = Some(child.as_or_node().expect("kind matched").right().kind());
        }
    });
    matches!(last_or_rhs, Some(NodeKind::NextNode | NodeKind::RedoNode))
}

impl Rule for UnreachableLoop {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UnreachableLoop",
        department: Department::Lint,
        summary: "Checks for loops that will have at most one iteration.",
        explanation: "\
Checks for loops that will have at most one iteration.

A loop that can never reach the second iteration is a possible error in the
code. In rare cases where only one iteration (or at most one iteration) is
intended behavior, the code should be refactored to use `if` conditionals.

NOTE: Block methods that are used with `Enumerable`s are considered to be
loops.

`AllowedPatterns` can be used to match against the block receiver in order
to allow code that would otherwise be registered as an offense (eg. `times`
used not in an `Enumerable` context).

```ruby
# bad
while node
  do_something(node)
  node = node.parent
  break
end

# good
while node
  do_something(node)
  node = node.parent
end

# bad
def verify_list(head)
  item = head
  begin
    if verify(item)
      return true
    else
      return false
    end
  end while(item)
end

# good
def verify_list(head)
  item = head
  begin
    if verify(item)
      item = item.next
    else
      return false
    end
  end while(item)

  true
end

# bad
def find_something(items)
  items.each do |item|
    if something?(item)
      return item
    else
      raise NotFoundError
    end
  end
end

# good
def find_something(items)
  items.each do |item|
    if something?(item)
      return item
    end
  end
  raise NotFoundError
end

# bad
2.times { raise ArgumentError }
```

With `AllowedPatterns: ['(exactly|at_least|at_most)\\(\\d+\\)\\.times']` (the default):

```ruby
# good
exactly(2).times { raise StandardError }
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::WhileNode,
            NodeKind::UntilNode,
            NodeKind::ForNode,
            NodeKind::BlockNode,
            NodeKind::CallNode,
        ],
        config: &[ConfigOption {
            name: "AllowedPatterns",
            default: ConfigDefault::StrList(&[r"(exactly|at_least|at_most)\(\d+\)\.times"]),
            allowed: &[],
            doc: "Method call source patterns (matched against the loop-like block's owning \
                  call, excluding the block itself) that are never flagged.",
        }],
        blind_spots: "\
`conditional_continue_keyword?`'s `each_descendant(:or).to_a.last` takes the \
last `or` node in traversal (pre-)order, which for a chain of more than one \
`||` is not necessarily the last one written in source (every fixture has \
at most one `||`, so this never diverges in practice).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let allowed_patterns = options
            .str_list("AllowedPatterns")
            .iter()
            .filter_map(|pattern| Regex::new(pattern).ok())
            .collect();
        Ok(Self { allowed_patterns, block_call_stack: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::WhileNode => {
                let n = node.as_while_node().expect("kind matched");
                self.check(Body::from_statements(n.statements()), node.span(), ctx);
            }
            NodeKind::UntilNode => {
                let n = node.as_until_node().expect("kind matched");
                self.check(Body::from_statements(n.statements()), node.span(), ctx);
            }
            NodeKind::ForNode => {
                let n = node.as_for_node().expect("kind matched");
                self.check(Body::from_statements(n.statements()), node.span(), ctx);
            }
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                if matches!(call.block(), Some(Node::BlockNode { .. })) {
                    let report_span =
                        self.is_loop_method_call(&call, ctx).then(|| call.as_node().span());
                    self.block_call_stack.push(report_span);
                }
            }
            NodeKind::BlockNode => {
                if let Some(&Some(report_span)) = self.block_call_stack.last() {
                    let n = node.as_block_node().expect("kind matched");
                    self.check(block_body(n.body()), report_span, ctx);
                }
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.kind() == NodeKind::CallNode {
            let call = node.as_call_node().expect("kind matched");
            if matches!(call.block(), Some(Node::BlockNode { .. })) {
                self.block_call_stack.pop();
            }
        }
    }
}
