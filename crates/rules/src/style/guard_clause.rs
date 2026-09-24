//! `Style/GuardClause`, ported from RuboCop's `lib/rubocop/cop/style/guard_clause.rb`
//! plus its `MinBodyLength` and `StatementModifier` mixins.

use std::collections::HashSet;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::StatementsNode;
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
fn message(example: &str) -> String {
    format!(
        "Use a guard clause (`{example}`) instead of wrapping the code inside a conditional \
         expression."
    )
}

/// Looks for conditionals that can be replaced with guard clauses.
#[derive(Debug, Clone)]
pub struct GuardClause {
    min_body_length: i64,
    allow_consecutive_conditionals: bool,
    /// `Layout/LineLength`'s configured `Max`, or `None` when that cop (or
    /// its department) is disabled -- RuboCop's `AutocorrectLogic#max_line_length`.
    max_line_length: Option<i64>,
    /// Byte offsets of `if`/`unless` nodes that are the value of a simple
    /// assignment (`result = if ... end`), so `on_if` can skip them the way
    /// RuboCop's `node.parent&.assignment?` does.
    assigned_if_starts: HashSet<u32>,
    /// `(span, name)` for every `LocalVariableWriteNode` seen so far this
    /// file, in source order, so `assigned_lvar_used_in_if_branch` can query
    /// a bounded span range instead of walking the if-node's subtree.
    local_writes: Vec<(Span, Box<[u8]>)>,
    /// `(span, name)` for every `LocalVariableReadNode` seen so far this
    /// file, in source order.
    local_reads: Vec<(Span, Box<[u8]>)>,
}

impl Rule for GuardClause {
    const META: RuleMeta = RuleMeta {
        name: "Style/GuardClause",
        department: Department::Style,
        summary: "Checks for conditionals that can be replaced with guard clauses.",
        explanation: "\
A condition with an `elsif` or `else` branch is allowed unless one of
`return`, `break`, `next`, `raise`, or `fail` is used in the body of the
conditional expression.

```ruby
# bad
def test
  if something
    work
  end
end

# good
def test
  return unless something

  work
end

# also good
def test
  work if something
end

# bad
if something
  raise 'exception'
else
  ok
end

# good
raise 'exception' if something
ok

# bad
define_method(:test) do
  if something
    work
  end
end

# good
define_method(:test) do
  return unless something

  work
end

# also good
define_method(:test) do
  work if something
end
```

With `AllowConsecutiveConditionals: true`, an ending `if`/`unless` directly
preceded by another `if`/`unless` statement (with no intervening code) is
not flagged.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::DefNode,
            NodeKind::CallNode,
            NodeKind::IfNode,
            NodeKind::UnlessNode,
            NodeKind::LocalVariableWriteNode,
            NodeKind::LocalVariableAndWriteNode,
            NodeKind::LocalVariableOrWriteNode,
            NodeKind::LocalVariableOperatorWriteNode,
            NodeKind::LocalVariableReadNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::InstanceVariableAndWriteNode,
            NodeKind::InstanceVariableOrWriteNode,
            NodeKind::InstanceVariableOperatorWriteNode,
            NodeKind::ClassVariableWriteNode,
            NodeKind::ClassVariableAndWriteNode,
            NodeKind::ClassVariableOrWriteNode,
            NodeKind::ClassVariableOperatorWriteNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::GlobalVariableAndWriteNode,
            NodeKind::GlobalVariableOrWriteNode,
            NodeKind::GlobalVariableOperatorWriteNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantAndWriteNode,
            NodeKind::ConstantOrWriteNode,
            NodeKind::ConstantOperatorWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::ConstantPathAndWriteNode,
            NodeKind::ConstantPathOrWriteNode,
            NodeKind::ConstantPathOperatorWriteNode,
            NodeKind::CallAndWriteNode,
            NodeKind::CallOrWriteNode,
            NodeKind::CallOperatorWriteNode,
            NodeKind::IndexAndWriteNode,
            NodeKind::IndexOrWriteNode,
            NodeKind::IndexOperatorWriteNode,
            NodeKind::MultiWriteNode,
        ],
        config: &[
            ConfigOption {
                name: "MinBodyLength",
                default: ConfigDefault::Int(1),
                allowed: &[],
                doc: "The number of lines a conditional's body needs to trigger this cop.",
            },
            ConfigOption {
                name: "AllowConsecutiveConditionals",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Allow an ending `if`/`unless` directly preceded by another one.",
            },
        ],
        blind_spots: "\
`node.parent&.assignment?` (RuboCop skips an `if`/`unless` used as the value
of an assignment) is approximated by tracking every simple and compound
assignment kind (`=`, `+=`, `||=`, `&&=`) over local/instance/class/global
variables, constants, constant paths, attribute writers, index writers, and
multiple assignment.
`node.method?(:define_method)` does not check the call's receiver, matching
RuboCop, so `obj.define_method(...) do ... end` is treated the same as a
bare call.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let min_body_length = options.int("MinBodyLength");
        let min_body_length = if min_body_length > 0 { min_body_length } else { 1 };
        let max_line_length = {
            let enabled = options
                .peer("Layout/LineLength", "Enabled")
                .and_then(OptionValue::as_bool)
                .unwrap_or(true);
            enabled.then(|| {
                options
                    .peer("Layout/LineLength", "Max")
                    .and_then(OptionValue::as_int)
                    .unwrap_or(120)
            })
        };
        Ok(Self {
            min_body_length,
            allow_consecutive_conditionals: options.bool("AllowConsecutiveConditionals"),
            max_line_length,
            assigned_if_starts: HashSet::new(),
            local_writes: Vec::new(),
            local_reads: Vec::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.assigned_if_starts.clear();
        self.local_writes.clear();
        self.local_reads.clear();
    }

    fn enter(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node {
            Node::LocalVariableWriteNode { .. } => {
                let n = node.as_local_variable_write_node().expect("kind matched");
                self.note_assignment(&n.value());
                self.local_writes.push((
                    node.location().span(),
                    n.name_loc().as_slice().to_vec().into_boxed_slice(),
                ));
            }
            Node::LocalVariableReadNode { .. } => {
                let n = node.as_local_variable_read_node().expect("kind matched");
                self.local_reads.push((
                    node.location().span(),
                    n.location().as_slice().to_vec().into_boxed_slice(),
                ));
            }
            Node::LocalVariableAndWriteNode { .. }
            | Node::LocalVariableOrWriteNode { .. }
            | Node::LocalVariableOperatorWriteNode { .. }
            | Node::InstanceVariableWriteNode { .. }
            | Node::InstanceVariableAndWriteNode { .. }
            | Node::InstanceVariableOrWriteNode { .. }
            | Node::InstanceVariableOperatorWriteNode { .. }
            | Node::ClassVariableWriteNode { .. }
            | Node::ClassVariableAndWriteNode { .. }
            | Node::ClassVariableOrWriteNode { .. }
            | Node::ClassVariableOperatorWriteNode { .. }
            | Node::GlobalVariableWriteNode { .. }
            | Node::GlobalVariableAndWriteNode { .. }
            | Node::GlobalVariableOrWriteNode { .. }
            | Node::GlobalVariableOperatorWriteNode { .. }
            | Node::ConstantWriteNode { .. }
            | Node::ConstantAndWriteNode { .. }
            | Node::ConstantOrWriteNode { .. }
            | Node::ConstantOperatorWriteNode { .. }
            | Node::ConstantPathWriteNode { .. }
            | Node::ConstantPathAndWriteNode { .. }
            | Node::ConstantPathOrWriteNode { .. }
            | Node::ConstantPathOperatorWriteNode { .. }
            | Node::CallAndWriteNode { .. }
            | Node::CallOrWriteNode { .. }
            | Node::CallOperatorWriteNode { .. }
            | Node::IndexAndWriteNode { .. }
            | Node::IndexOrWriteNode { .. }
            | Node::IndexOperatorWriteNode { .. }
            | Node::MultiWriteNode { .. } => {
                if let Some(value) = assignment_value(node) {
                    self.note_assignment(&value);
                }
            }
            _ => {}
        }
    }

    /// Runs the def-ending-body and general `on_if` checks after the
    /// node's own subtree -- including any local-variable reads/writes it
    /// contains, recorded by `enter` above -- has been fully visited, so
    /// `assigned_lvar_used_in_if_branch` can query `local_writes`/
    /// `local_reads` by span instead of walking the subtree here.
    fn leave(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::DefNode { .. } => {
                let n = node.as_def_node().expect("kind matched");
                self.check_ending_body(n.body(), ctx);
            }
            Node::CallNode { .. } => {
                let n = node.as_call_node().expect("kind matched");
                if matches!(n.name().as_slice(), b"define_method" | b"define_singleton_method") {
                    if let Some(block) = n.block() {
                        if let Node::BlockNode { .. } = &block {
                            let block = block.as_block_node().expect("kind matched");
                            self.check_ending_body(block.body(), ctx);
                        }
                    }
                }
            }
            Node::IfNode { .. } | Node::UnlessNode { .. } => self.handle_if(node, ctx),
            _ => {}
        }
    }
}

impl GuardClause {
    /// Records that `value` (an assignment's right-hand side) is an
    /// `if`/`unless` node, so [`accepted`] can treat it like RuboCop's
    /// `node.parent&.assignment?`.
    fn note_assignment(&mut self, value: &Node<'_>) {
        if matches!(value, Node::IfNode { .. } | Node::UnlessNode { .. }) {
            self.assigned_if_starts.insert(value.location().span().start);
        }
    }

    /// RuboCop's `on_if`: reports when an `if`/`unless` with a plain `else`
    /// (no `elsif`) has a guard clause (`return`/`break`/`next`/`raise`/`fail`)
    /// in one of its branches.
    fn handle_if(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(shape) = shape_of(node) else { return };
        if self.accepted(&shape, false, ctx) {
            return;
        }
        let (guard, kw, side) = if let Some(g) = guard_clause_of(&shape.if_branch, ctx) {
            (g, if shape.is_unless { "unless" } else { "if" }, Side::If)
        } else if let Some(g) = guard_clause_of(&shape.else_branch, ctx) {
            (g, if shape.is_unless { "if" } else { "unless" }, Side::Else)
        } else {
            return;
        };
        let side = if guard.is_and_or { None } else { Some(side) };
        let scope_exiting = ctx.text(guard.text_span).to_vec();
        register_offense(self, ctx, &shape, &scope_exiting, kw, side);
    }

    /// RuboCop's `check_ending_body`.
    fn check_ending_body(&self, body: Option<Node<'_>>, ctx: &mut Context<'_>) {
        let Some(body) = body else { return };
        match &body {
            Node::IfNode { .. } | Node::UnlessNode { .. } => {
                self.check_ending_if(&body, false, ctx);
            }
            Node::StatementsNode { .. } => {
                let s = body.as_statements_node().expect("kind matched");
                let items: Vec<Node<'_>> = s.body().iter().collect();
                if let Some(last) = items.last() {
                    if matches!(last, Node::IfNode { .. } | Node::UnlessNode { .. }) {
                        let prev_is_if = items.len() >= 2
                            && matches!(
                                items[items.len() - 2],
                                Node::IfNode { .. } | Node::UnlessNode { .. }
                            );
                        self.check_ending_if(last, prev_is_if, ctx);
                    }
                }
            }
            _ => {}
        }
    }

    /// RuboCop's `check_ending_if`: the `if`/`unless` that is the final
    /// statement of a method/block body, without an `else`, always exits
    /// with `return`.
    fn check_ending_if(&self, node: &Node<'_>, prev_is_if: bool, ctx: &mut Context<'_>) {
        let Some(shape) = shape_of(node) else { return };
        if self.accepted(&shape, true, ctx) {
            return;
        }
        if !min_body_length_ok(&shape, self.min_body_length, ctx) {
            return;
        }
        if self.allow_consecutive_conditionals && prev_is_if {
            return;
        }
        register_offense(
            self,
            ctx,
            &shape,
            b"return",
            if shape.is_unless { "if" } else { "unless" },
            None,
        );
        let sub_body = match shape.if_branch {
            Branch::Empty => None,
            Branch::Single(n) => Some(n),
            Branch::Multi(s) => Some(s.as_node()),
        };
        self.check_ending_body(sub_body, ctx);
    }

    /// RuboCop's `accepted_form?`.
    fn accepted(&self, shape: &Shape<'_>, ending: bool, ctx: &Context<'_>) -> bool {
        self.accepted_if(shape, ending)
            || predicate_multiline(shape, ctx)
            || self.assigned_if_starts.contains(&shape.node.location().span().start)
    }

    /// RuboCop's `accepted_if?`.
    fn accepted_if(&self, shape: &Shape<'_>, ending: bool) -> bool {
        let modifier_form = shape.end_span.is_none();
        if modifier_form
            || elsif_conditional(&shape.else_branch)
            || self.assigned_lvar_used_in_if_branch(shape)
        {
            return true;
        }
        if ending {
            shape.has_else
        } else {
            !shape.has_else || shape.is_elsif
        }
    }

    /// RuboCop's `assigned_lvar_used_in_if_branch?`, querying the
    /// `local_writes`/`local_reads` recorded by `enter` instead of walking
    /// the predicate/branch subtrees here.
    fn assigned_lvar_used_in_if_branch(&self, shape: &Shape<'_>) -> bool {
        if matches!(shape.if_branch, Branch::Empty) {
            return false;
        }
        let predicate_span = shape.predicate.location().span();
        let mut assigned = names_within(&self.local_writes, predicate_span).peekable();
        if assigned.peek().is_none() {
            return false;
        }
        let Some(used_span) = branch_span(&shape.if_branch) else { return false };
        let used: HashSet<&[u8]> = names_within(&self.local_reads, used_span).collect();
        assigned.any(|name| used.contains(name))
    }
}

/// Extracts the assigned value from every assignment-node kind besides
/// `LocalVariableWriteNode` (handled separately in `enter` so it can also
/// record `local_writes`). Covers rubocop-ast's `ASSIGNMENTS`: the plain
/// `=` forms (`lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`/`casgn`/`masgn`) plus the
/// generic `op_asgn`/`or_asgn`/`and_asgn` compound forms, which in
/// whitequark's AST apply uniformly to variables, constants, attribute
/// writers (`obj.foo += 1`), and index writers (`arr[0] ||= 1`) alike.
fn assignment_value<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    match node {
        Node::LocalVariableAndWriteNode { .. } => {
            Some(node.as_local_variable_and_write_node().expect("kind matched").value())
        }
        Node::LocalVariableOrWriteNode { .. } => {
            Some(node.as_local_variable_or_write_node().expect("kind matched").value())
        }
        Node::LocalVariableOperatorWriteNode { .. } => {
            Some(node.as_local_variable_operator_write_node().expect("kind matched").value())
        }
        Node::InstanceVariableWriteNode { .. } => {
            Some(node.as_instance_variable_write_node().expect("kind matched").value())
        }
        Node::InstanceVariableAndWriteNode { .. } => {
            Some(node.as_instance_variable_and_write_node().expect("kind matched").value())
        }
        Node::InstanceVariableOrWriteNode { .. } => {
            Some(node.as_instance_variable_or_write_node().expect("kind matched").value())
        }
        Node::InstanceVariableOperatorWriteNode { .. } => {
            Some(node.as_instance_variable_operator_write_node().expect("kind matched").value())
        }
        Node::ClassVariableWriteNode { .. } => {
            Some(node.as_class_variable_write_node().expect("kind matched").value())
        }
        Node::ClassVariableAndWriteNode { .. } => {
            Some(node.as_class_variable_and_write_node().expect("kind matched").value())
        }
        Node::ClassVariableOrWriteNode { .. } => {
            Some(node.as_class_variable_or_write_node().expect("kind matched").value())
        }
        Node::ClassVariableOperatorWriteNode { .. } => {
            Some(node.as_class_variable_operator_write_node().expect("kind matched").value())
        }
        Node::GlobalVariableWriteNode { .. } => {
            Some(node.as_global_variable_write_node().expect("kind matched").value())
        }
        Node::GlobalVariableAndWriteNode { .. } => {
            Some(node.as_global_variable_and_write_node().expect("kind matched").value())
        }
        Node::GlobalVariableOrWriteNode { .. } => {
            Some(node.as_global_variable_or_write_node().expect("kind matched").value())
        }
        Node::GlobalVariableOperatorWriteNode { .. } => {
            Some(node.as_global_variable_operator_write_node().expect("kind matched").value())
        }
        Node::ConstantWriteNode { .. } => {
            Some(node.as_constant_write_node().expect("kind matched").value())
        }
        Node::ConstantAndWriteNode { .. } => {
            Some(node.as_constant_and_write_node().expect("kind matched").value())
        }
        Node::ConstantOrWriteNode { .. } => {
            Some(node.as_constant_or_write_node().expect("kind matched").value())
        }
        Node::ConstantOperatorWriteNode { .. } => {
            Some(node.as_constant_operator_write_node().expect("kind matched").value())
        }
        Node::ConstantPathWriteNode { .. } => {
            Some(node.as_constant_path_write_node().expect("kind matched").value())
        }
        Node::ConstantPathAndWriteNode { .. } => {
            Some(node.as_constant_path_and_write_node().expect("kind matched").value())
        }
        Node::ConstantPathOrWriteNode { .. } => {
            Some(node.as_constant_path_or_write_node().expect("kind matched").value())
        }
        Node::ConstantPathOperatorWriteNode { .. } => {
            Some(node.as_constant_path_operator_write_node().expect("kind matched").value())
        }
        Node::CallAndWriteNode { .. } => {
            Some(node.as_call_and_write_node().expect("kind matched").value())
        }
        Node::CallOrWriteNode { .. } => {
            Some(node.as_call_or_write_node().expect("kind matched").value())
        }
        Node::CallOperatorWriteNode { .. } => {
            Some(node.as_call_operator_write_node().expect("kind matched").value())
        }
        Node::IndexAndWriteNode { .. } => {
            Some(node.as_index_and_write_node().expect("kind matched").value())
        }
        Node::IndexOrWriteNode { .. } => {
            Some(node.as_index_or_write_node().expect("kind matched").value())
        }
        Node::IndexOperatorWriteNode { .. } => {
            Some(node.as_index_operator_write_node().expect("kind matched").value())
        }
        Node::MultiWriteNode { .. } => {
            Some(node.as_multi_write_node().expect("kind matched").value())
        }
        _ => None,
    }
}

/// Which branch (of a plain `if`/`else`, no `elsif`) held the guard clause
/// that got folded into the single-line replacement.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    If,
    Else,
}

/// A normalized `if`/`unless` branch body, mirroring `rubocop-ast`'s
/// `IfNode#if_branch`/`#else_branch`: a single statement is unwrapped, an
/// empty body is `Empty`, and multiple statements stay as the underlying
/// `StatementsNode` (RuboCop's synthetic `begin`).
enum Branch<'pr> {
    Empty,
    Single(Node<'pr>),
    Multi(StatementsNode<'pr>),
}

fn branch_of(stmts: Option<StatementsNode<'_>>) -> Branch<'_> {
    let Some(stmts) = stmts else { return Branch::Empty };
    let items: Vec<Node<'_>> = stmts.body().iter().collect();
    match items.len() {
        0 => Branch::Empty,
        1 => Branch::Single(items.into_iter().next().expect("len == 1")),
        _ => Branch::Multi(stmts),
    }
}

fn branch_span(branch: &Branch<'_>) -> Option<Span> {
    match branch {
        Branch::Empty => None,
        Branch::Single(n) => Some(n.location().span()),
        Branch::Multi(s) => Some(s.as_node().location().span()),
    }
}

/// A normalized view of one `if`/`unless` node, spanning both Prism node
/// kinds (RuboCop's whitequark-based AST unifies them, Prism does not).
struct Shape<'pr> {
    node: Node<'pr>,
    is_unless: bool,
    keyword_span: Span,
    /// True when this node is itself an `elsif` continuation.
    is_elsif: bool,
    then_span: Option<Span>,
    predicate: Node<'pr>,
    end_span: Option<Span>,
    has_else: bool,
    /// The `else`/`elsif` keyword's span, present exactly when there is a
    /// genuine (non-`elsif`) `else` clause.
    else_span: Option<Span>,
    if_branch: Branch<'pr>,
    else_branch: Branch<'pr>,
}

/// Builds a [`Shape`] for an `IfNode`/`UnlessNode`. Returns `None` for a
/// ternary (`a ? b : c`, also an `IfNode` in Prism with no `if`/`elsif`
/// keyword), matching RuboCop's `ternary?` short-circuit in `accepted_if?`.
fn shape_of<'pr>(node: &Node<'pr>) -> Option<Shape<'pr>> {
    match node {
        Node::IfNode { .. } => {
            let n = node.as_if_node().expect("kind matched");
            let kw_loc = n.if_keyword_loc()?;
            let is_elsif = kw_loc.as_slice() == b"elsif";
            let end_span = n.end_keyword_loc().map(|l| l.span());
            let then_span = n.then_keyword_loc().map(|l| l.span());
            let if_branch = branch_of(n.statements());
            let (has_else, else_span, else_branch) = match n.subsequent() {
                None => (false, None, Branch::Empty),
                Some(sub) => match &sub {
                    Node::IfNode { .. } => (true, None, Branch::Single(sub)),
                    Node::ElseNode { .. } => {
                        let e = sub.as_else_node().expect("kind matched");
                        (true, Some(e.else_keyword_loc().span()), branch_of(e.statements()))
                    }
                    _ => unreachable!("IfNode#subsequent is an if or an else"),
                },
            };
            Some(Shape {
                node: node.as_if_node().expect("kind matched").as_node(),
                is_unless: false,
                keyword_span: kw_loc.span(),
                is_elsif,
                then_span,
                predicate: n.predicate(),
                end_span,
                has_else,
                else_span,
                if_branch,
                else_branch,
            })
        }
        Node::UnlessNode { .. } => {
            let n = node.as_unless_node().expect("kind matched");
            let end_span = n.end_keyword_loc().map(|l| l.span());
            let then_span = n.then_keyword_loc().map(|l| l.span());
            let if_branch = branch_of(n.statements());
            let (has_else, else_span, else_branch) = match n.else_clause() {
                None => (false, None, Branch::Empty),
                Some(e) => (true, Some(e.else_keyword_loc().span()), branch_of(e.statements())),
            };
            Some(Shape {
                node: n.as_node(),
                is_unless: true,
                keyword_span: n.keyword_loc().span(),
                is_elsif: false,
                then_span,
                predicate: n.predicate(),
                end_span,
                has_else,
                else_span,
                if_branch,
                else_branch,
            })
        }
        _ => None,
    }
}

/// RuboCop's `IfNode#elsif_conditional?`.
fn elsif_conditional(else_branch: &Branch<'_>) -> bool {
    let Branch::Single(n) = else_branch else { return false };
    let Node::IfNode { .. } = n else { return false };
    let inner = n.as_if_node().expect("kind matched");
    inner.if_keyword_loc().is_some_and(|kw| kw.as_slice() == b"elsif")
}

/// RuboCop's `Node#condition.multiline?`.
fn predicate_multiline(shape: &Shape<'_>, ctx: &Context<'_>) -> bool {
    let span = shape.predicate.location().span();
    let last = if span.end > span.start { span.end - 1 } else { span.start };
    ctx.line_col(span.start).line != ctx.line_col(last).line
}

/// Names among `pairs` (source-ordered `(span, name)`, as recorded by
/// [`GuardClause::enter`]) whose span lies strictly within `range`, located
/// with a binary search instead of a subtree walk. Mirrors `each_descendant`:
/// a pair whose span exactly equals `range` is the root node itself (not a
/// descendant) and is excluded, matching RuboCop's `node.condition
/// .each_descendant(:lvasgn)` skipping a condition that is itself an
/// assignment (`if x = foo`).
fn names_within(pairs: &[(Span, Box<[u8]>)], range: Span) -> impl Iterator<Item = &[u8]> {
    let start = pairs.partition_point(|(span, _)| span.start < range.start);
    pairs[start..]
        .iter()
        .take_while(move |(span, _)| span.start < range.end)
        .filter(move |(span, _)| span.end <= range.end && *span != range)
        .map(|(_, name)| name.as_ref())
}

/// A matched guard clause: the span whose source text is used to build the
/// message/replacement (the `return`/`raise ...` text, or the whole `and`/`or`
/// expression when wrapped), and whether it was and/or-wrapped.
struct GuardMatch {
    text_span: Span,
    is_and_or: bool,
}

/// RuboCop's `Node#guard_clause?`, called on a branch.
fn guard_clause_of(branch: &Branch<'_>, ctx: &Context<'_>) -> Option<GuardMatch> {
    let Branch::Single(n) = branch else { return None };
    if let Node::AndNode { .. } | Node::OrNode { .. } = n {
        let rhs = and_or_rhs(n);
        if is_guard_exit(&rhs) && single_line(&rhs, ctx) {
            return Some(GuardMatch { text_span: n.location().span(), is_and_or: true });
        }
        return None;
    }
    if is_guard_exit(n) && single_line(n, ctx) {
        return Some(GuardMatch { text_span: n.location().span(), is_and_or: false });
    }
    None
}

fn and_or_rhs<'pr>(n: &Node<'pr>) -> Node<'pr> {
    match n {
        Node::AndNode { .. } => n.as_and_node().expect("kind matched").right(),
        Node::OrNode { .. } => n.as_or_node().expect("kind matched").right(),
        _ => unreachable!("caller matched And/Or"),
    }
}

/// RuboCop's `match_guard_clause?`: a bare `raise`/`fail` call, or a
/// `return`/`break`/`next`.
fn is_guard_exit(n: &Node<'_>) -> bool {
    match n {
        Node::ReturnNode { .. } | Node::BreakNode { .. } | Node::NextNode { .. } => true,
        Node::CallNode { .. } => {
            let c = n.as_call_node().expect("kind matched");
            c.receiver().is_none() && matches!(c.name().as_slice(), b"raise" | b"fail")
        }
        _ => false,
    }
}

fn single_line(n: &Node<'_>, ctx: &Context<'_>) -> bool {
    let span = n.location().span();
    let last = if span.end > span.start { span.end - 1 } else { span.start };
    ctx.line_col(span.start).line == ctx.line_col(last).line
}

/// RuboCop's `MinBodyLength#min_body_length?`.
fn min_body_length_ok(shape: &Shape<'_>, min: i64, ctx: &Context<'_>) -> bool {
    let Some(end_span) = shape.end_span else { return false };
    let end_line = i64::from(ctx.line_col(end_span.start).line);
    let kw_line = i64::from(ctx.line_col(shape.keyword_span.start).line);
    end_line - kw_line > min
}

/// RuboCop's `trivial?`.
fn trivial(shape: &Shape<'_>) -> bool {
    if shape.has_else {
        return false;
    }
    match &shape.if_branch {
        Branch::Single(n) => !matches!(n, Node::IfNode { .. } | Node::UnlessNode { .. }),
        Branch::Empty | Branch::Multi(_) => false,
    }
}

/// RuboCop's `register_offense`.
fn register_offense(
    rule: &GuardClause,
    ctx: &mut Context<'_>,
    shape: &Shape<'_>,
    scope_exiting: &[u8],
    conditional_kw: &str,
    guard_side: Option<Side>,
) {
    let condition_span = shape.predicate.location().span();
    let condition_text = String::from_utf8_lossy(ctx.text(condition_span)).into_owned();
    let scope_exiting_text = String::from_utf8_lossy(scope_exiting).into_owned();
    let example = format!("{scope_exiting_text} {conditional_kw} {condition_text}");

    let node_span = shape.node.location().span();
    let column = i64::from(ctx.line_col(node_span.start).column);
    let too_long = rule.max_line_length.is_some_and(|max| {
        column + i64::try_from(example.chars().count()).unwrap_or(i64::MAX) > max
    });

    let (message_example, replacement) = if too_long {
        if trivial(shape) {
            return;
        }
        let message_example =
            format!("{conditional_kw} {condition_text}; {scope_exiting_text}; end");
        let replacement = format!("{conditional_kw} {condition_text}\n  {scope_exiting_text}\nend");
        (message_example, replacement)
    } else {
        (example.clone(), example)
    };

    let msg = message(&message_example);

    if shape.has_else && guard_side.is_none() {
        ctx.report(&GuardClause::META, shape.keyword_span, msg);
        return;
    }

    let fix = build_fix(shape, &replacement, guard_side, ctx);
    ctx.report_with_fix(&GuardClause::META, shape.keyword_span, msg, fix);
}

/// RuboCop's `autocorrect`.
fn build_fix(
    shape: &Shape<'_>,
    replacement: &str,
    guard_side: Option<Side>,
    ctx: &Context<'_>,
) -> Fix {
    let mut edits = Vec::new();

    let predicate_end = shape.predicate.location().span().end;
    edits.push(Edit::replace(
        Span::new(shape.keyword_span.start, predicate_end),
        replacement.as_bytes().to_vec(),
    ));
    if let Some(then_span) = shape.then_span {
        edits.push(Edit::replace(then_span, b"\n".to_vec()));
    }

    let heredoc = heredoc_branch(&shape.if_branch, ctx)
        .map(|info| (info, &shape.else_branch))
        .or_else(|| heredoc_branch(&shape.else_branch, ctx).map(|info| (info, &shape.if_branch)));

    if let Some((info, leave_branch)) = heredoc {
        let end_span = shape.end_span.expect("if/unless with a body has an end keyword");
        edits.push(Edit::delete(whole_lines_span(ctx, end_span)));
        if shape.has_else {
            if let Some(span) = branch_span(leave_branch) {
                edits.push(Edit::delete(whole_lines_span(ctx, span)));
            }
            let else_span = shape.else_span.expect("has_else implies an else span");
            edits.push(Edit::delete(whole_lines_span(ctx, else_span)));
            let removed = match guard_side {
                Some(Side::If) => &shape.if_branch,
                Some(Side::Else) => &shape.else_branch,
                None => unreachable!("build_fix only removes a branch when a guard side is known"),
            };
            if let Some(span) = branch_span(removed) {
                edits.push(Edit::delete(whole_lines_span(ctx, span)));
            }
            if let Some(leave_span) = branch_span(leave_branch) {
                let mut insert_text = b"\n".to_vec();
                insert_text.extend_from_slice(ctx.text(leave_span));
                edits.push(Edit::insert(info.closing_end, insert_text));
            }
        }
    } else {
        let end_span = shape.end_span.expect("if/unless with a body has an end keyword");
        edits.push(Edit::delete(end_span));
        if shape.has_else {
            let else_span = shape.else_span.expect("has_else implies an else span");
            edits.push(Edit::delete(else_span));
            let removed = match guard_side {
                Some(Side::If) => &shape.if_branch,
                Some(Side::Else) => &shape.else_branch,
                None => unreachable!("build_fix only removes a branch when a guard side is known"),
            };
            if let Some(span) = branch_span(removed) {
                edits.push(Edit::delete(span));
            }
        }
    }

    Fix { applicability: Applicability::Safe, edits }
}

/// One heredoc argument found as a branch's whole body: RuboCop's
/// `if_branch.send_type? && heredoc?(if_branch.last_argument)`, carrying the
/// heredoc's closing-delimiter end offset for `insert_after`.
struct HeredocInfo {
    closing_end: u32,
}

fn heredoc_branch(branch: &Branch<'_>, ctx: &Context<'_>) -> Option<HeredocInfo> {
    let Branch::Single(n) = branch else { return None };
    let call = n.as_call_node()?;
    let args = call.arguments()?;
    let last = args.arguments().last()?;
    let (opening, closing) = string_heredoc_locs(&last)?;
    if !ctx.text(opening).starts_with(b"<<") {
        return None;
    }
    // Prism's heredoc closing location includes the trailing line
    // terminator (`"MESSAGE\n"`); RuboCop's `loc.heredoc_end` (parser gem)
    // stops before it, which is where `insert_after` must land.
    let closing_text = ctx.text(closing);
    let trimmed = closing_text.len()
        - closing_text.iter().rev().take_while(|&&b| b == b'\n' || b == b'\r').count();
    let closing_end = closing.start + u32::try_from(trimmed).unwrap_or(closing.len());
    Some(HeredocInfo { closing_end })
}

/// Opening/closing delimiter spans for a heredoc-capable string literal.
fn string_heredoc_locs(n: &Node<'_>) -> Option<(Span, Span)> {
    match n {
        Node::StringNode { .. } => {
            let s = n.as_string_node().expect("kind matched");
            Some((s.opening_loc()?.span(), s.closing_loc()?.span()))
        }
        Node::InterpolatedStringNode { .. } => {
            let s = n.as_interpolated_string_node().expect("kind matched");
            Some((s.opening_loc()?.span(), s.closing_loc()?.span()))
        }
        Node::XStringNode { .. } => {
            let s = n.as_x_string_node().expect("kind matched");
            Some((s.opening_loc().span(), s.closing_loc().span()))
        }
        Node::InterpolatedXStringNode { .. } => {
            let s = n.as_interpolated_x_string_node().expect("kind matched");
            Some((s.opening_loc().span(), s.closing_loc().span()))
        }
        _ => None,
    }
}

/// RuboCop's `range_by_whole_lines(range, include_final_newline: true)`.
fn whole_lines_span(ctx: &Context<'_>, span: Span) -> Span {
    let start_line = ctx.line_col(span.start).line;
    let last_included = if span.end > span.start { span.end - 1 } else { span.start };
    let end_line = ctx.line_col(last_included).line;
    let start = ctx.line_span(start_line).start;
    let line_end = ctx.line_span(end_line).end;
    let source_len = u32::try_from(ctx.source().bytes().len()).unwrap_or(u32::MAX);
    let end = if line_end < source_len { line_end + 1 } else { line_end };
    Span::new(start, end)
}
