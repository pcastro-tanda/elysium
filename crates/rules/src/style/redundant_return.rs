//! `Style/RedundantReturn`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_return.rb`.
//!
//! RuboCop's `on_def`/`on_send` callbacks each invoke a hand-rolled
//! `check_branch` recursion over the whitequark AST that follows only the
//! "last executed expression" path of a method/block body (through
//! `if`/`case`/`rescue`/`begin` branches) looking for a `return`. That
//! recursion is bounded -- it never revisits a subtree the engine's own
//! traversal will reach independently (nested `def`s, blocks, etc. are not
//! node kinds `check_node` below dispatches on) -- so it is reproduced here
//! as a direct, non-revisiting walk over Prism's node accessors from each
//! `DefNode`/`LambdaNode`/qualifying `CallNode` entry point, rather than by
//! subscribing every branch kind to the engine's `enter`/`leave` and
//! re-deriving context; this keeps the whole file to a single traversal.
//!
//! Prism splits whitequark's uniform `:begin`/`:kwbegin` (a plain statement
//! sequence) and its `:rescue`/`:resbody`/`:ensure` (attached to the same
//! node via `body`/`resbody_branches`/`else_branch`) into distinct node
//! kinds: an implicit multi-statement body is a bare [`StatementsNode`],
//! while one with a `rescue`/`else`/`ensure` clause is a [`BeginNode`]
//! carrying those clauses as separate optional fields. `check_begin` below
//! reconstructs RuboCop-AST's `RescueNode#branches` (each `resbody`'s body,
//! plus the `else` branch when present) and `check_rescue_node`'s
//! "check the protected body unless there's an else" from those fields
//! directly; an `ensure` clause's own statements are never the def's return
//! value, so [`BeginNode::ensure_clause`] is intentionally never examined.
//! `if`/`unless` are two Prism node kinds where whitequark has one
//! (`:if`, branches possibly swapped for `unless`); both are dispatched
//! here, with an `elsif` chain reached via `IfNode::subsequent` (itself
//! either another `IfNode` or an `ElseNode`) recursing back through the
//! same dispatcher.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{
    BeginNode, CaseMatchNode, CaseNode, ElseNode, IfNode, RescueNode, ReturnNode, StatementsNode,
    UnlessNode,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Redundant `return` detected.";
/// RuboCop's `MULTI_RETURN_MSG`.
const MULTI_RETURN_MSG: &str = "To return multiple values, use an array.";

/// Looks for redundant `return` expressions in the last executed
/// expression of a method, `lambda`, `define_method`/`define_singleton_method`
/// body, or `->` literal.
#[derive(Debug, Clone)]
pub struct RedundantReturn {
    allow_multiple_return_values: bool,
}

impl Rule for RedundantReturn {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantReturn",
        department: Department::Style,
        summary: "Don't use return where it's not required.",
        explanation: "\
A `return` at the end of a method, or as the last executed expression of an
`if`/`case`/`begin`/`rescue` branch within one, is unnecessary: the value
of the last expression is already the method's return value.

```ruby
# bad
def test
  return something
end

# bad
def test
  one
  two
  three
  return something
end

# bad
def test
  return something if something_else
end

# good
def test
  something if something_else
end

# good
def test
  if x
  elsif y
  else
  end
end
```

The same check applies to `define_method`/`define_singleton_method`/`lambda`
blocks and `->` literals.

@example AllowMultipleReturnValues: false (default)
```ruby
# bad
def test
  return x, y
end
```

@example AllowMultipleReturnValues: true
```ruby
# good
def test
  return x, y
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::DefNode, NodeKind::LambdaNode],
        config: &[ConfigOption {
            name: "AllowMultipleReturnValues",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "When `true`, allows code like `return x, y`.",
        }],
        blind_spots: "\
Matches RuboCop's own scope exactly: `define_method`/`define_singleton_method`/
`lambda` are matched by bare method name regardless of receiver, so
`Foo.lambda { return x }` is flagged too (an upstream imprecision, preserved
here for parity, not a gap specific to this port). The splat-argument
autocorrect only strips a leading `*` off the *first* return value (mirroring
RuboCop's own `first_argument`-based correction), so `return *a, b` where the
splat is not literally the first value is left with its `*` (also inherited
from upstream, not a new limitation).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_multiple_return_values: options.bool("AllowMultipleReturnValues") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::DefNode { .. } => {
                let n = node.as_def_node().expect("kind matched");
                check_body(n.body(), self, ctx);
            }
            Node::LambdaNode { .. } => {
                let n = node.as_lambda_node().expect("kind matched");
                check_body(n.body(), self, ctx);
            }
            Node::CallNode { .. } => {
                let n = node.as_call_node().expect("kind matched");
                if matches!(
                    n.name().as_slice(),
                    b"define_method" | b"define_singleton_method" | b"lambda"
                ) {
                    // RuboCop's `block_literal?`: a literal `do...end`/`{}`
                    // block, not a `&block` pass-through argument.
                    if let Some(block) = n.block().and_then(|b| b.as_block_node()) {
                        check_body(block.body(), self, ctx);
                    }
                }
            }
            _ => {}
        }
    }
}

/// RuboCop's `check_branch`, entered at a method/block body root: `body` is
/// `None` for an empty body, else a [`StatementsNode`] (plain statement
/// sequence) or a [`BeginNode`] (has a `rescue`/`else`/`ensure` clause).
fn check_body(body: Option<Node<'_>>, rule: &RedundantReturn, ctx: &mut Context<'_>) {
    if let Some(node) = body {
        check_node(&node, rule, ctx);
    }
}

/// RuboCop's `check_branch` dispatch table.
fn check_node(node: &Node<'_>, rule: &RedundantReturn, ctx: &mut Context<'_>) {
    match node {
        Node::ReturnNode { .. } => {
            check_return(&node.as_return_node().expect("kind matched"), rule, ctx);
        }
        Node::CaseNode { .. } => {
            check_case(&node.as_case_node().expect("kind matched"), rule, ctx);
        }
        Node::CaseMatchNode { .. } => {
            check_case_match(&node.as_case_match_node().expect("kind matched"), rule, ctx);
        }
        Node::IfNode { .. } => check_if(&node.as_if_node().expect("kind matched"), rule, ctx),
        Node::UnlessNode { .. } => {
            check_unless(&node.as_unless_node().expect("kind matched"), rule, ctx);
        }
        Node::ElseNode { .. } => {
            let e = node.as_else_node().expect("kind matched");
            check_statements(e.statements(), rule, ctx);
        }
        Node::BeginNode { .. } => {
            check_begin(&node.as_begin_node().expect("kind matched"), rule, ctx);
        }
        Node::StatementsNode { .. } => {
            check_last(&node.as_statements_node().expect("kind matched"), rule, ctx);
        }
        _ => {}
    }
}

/// RuboCop's `check_begin_node`: the last statement of an implicit
/// (unwrapped) statement sequence is itself dispatched through
/// [`check_node`], since it may be another branch construct.
fn check_last(stmts: &StatementsNode<'_>, rule: &RedundantReturn, ctx: &mut Context<'_>) {
    if let Some(last) = stmts.body().iter().last() {
        check_node(&last, rule, ctx);
    }
}

/// `check_last`, tolerating a branch with no body at all (an empty `if`/
/// `when`/`in`/`rescue`/`else` clause never blows up, matching RuboCop's
/// `check_branch(nil)` no-op).
fn check_statements(
    stmts: Option<StatementsNode<'_>>,
    rule: &RedundantReturn,
    ctx: &mut Context<'_>,
) {
    if let Some(stmts) = stmts {
        check_last(&stmts, rule, ctx);
    }
}

/// RuboCop's `check_if_node`, generalized to both `IfNode` and `UnlessNode`.
/// A ternary (`a ? b : c`, an `IfNode` with no `if`/`elsif` keyword) is
/// skipped, matching `node.ternary?`.
fn check_if(n: &IfNode<'_>, rule: &RedundantReturn, ctx: &mut Context<'_>) {
    if n.if_keyword_loc().is_none() {
        return;
    }
    check_statements(n.statements(), rule, ctx);
    // `subsequent` is an `elsif` (another `IfNode`) or an `else` clause (an
    // `ElseNode`); both are handled by re-entering the dispatcher.
    if let Some(sub) = n.subsequent() {
        check_node(&sub, rule, ctx);
    }
}

fn check_unless(n: &UnlessNode<'_>, rule: &RedundantReturn, ctx: &mut Context<'_>) {
    check_statements(n.statements(), rule, ctx);
    if let Some(e) = n.else_clause() {
        check_statements(e.statements(), rule, ctx);
    }
}

/// RuboCop's `check_case_node`.
fn check_case(n: &CaseNode<'_>, rule: &RedundantReturn, ctx: &mut Context<'_>) {
    for cond in &n.conditions() {
        if let Some(when) = cond.as_when_node() {
            check_statements(when.statements(), rule, ctx);
        }
    }
    if let Some(e) = n.else_clause() {
        check_statements(e.statements(), rule, ctx);
    }
}

/// RuboCop's `check_case_match_node`.
fn check_case_match(n: &CaseMatchNode<'_>, rule: &RedundantReturn, ctx: &mut Context<'_>) {
    for cond in &n.conditions() {
        if let Some(pattern) = cond.as_in_node() {
            check_statements(pattern.statements(), rule, ctx);
        }
    }
    if let Some(e) = n.else_clause() {
        check_statements(e.statements(), rule, ctx);
    }
}

/// RuboCop's `check_begin_node`/`check_rescue_node`/`check_resbody_node`/
/// `check_ensure_node` collapsed onto Prism's flat [`BeginNode`] fields: an
/// `ensure` clause's own body is never the def's return value (whether or
/// not a `rescue` is also present), so [`BeginNode::ensure_clause`] is
/// never read here.
fn check_begin(n: &BeginNode<'_>, rule: &RedundantReturn, ctx: &mut Context<'_>) {
    if let Some(rescue) = n.rescue_clause() {
        check_rescue_chain(rescue, n.statements(), n.else_clause(), rule, ctx);
    } else {
        check_statements(n.statements(), rule, ctx);
    }
}

/// RuboCop's `RescueNode#branches` (each `resbody`'s body, plus the `else`
/// branch if present) followed by `check_branch(node.body) unless
/// node.else?`: the protected body is checked only when there is no
/// `else`, since an `else` clause -- not the protected body -- is what
/// actually runs (and returns) when no exception was raised.
fn check_rescue_chain(
    head: RescueNode<'_>,
    protected: Option<StatementsNode<'_>>,
    else_clause: Option<ElseNode<'_>>,
    rule: &RedundantReturn,
    ctx: &mut Context<'_>,
) {
    let mut current = Some(head);
    while let Some(r) = current {
        check_statements(r.statements(), rule, ctx);
        current = r.subsequent();
    }
    match else_clause {
        Some(e) => check_statements(e.statements(), rule, ctx),
        None => check_statements(protected, rule, ctx),
    }
}

/// RuboCop's `check_return_node`/`message`.
fn check_return(n: &ReturnNode<'_>, rule: &RedundantReturn, ctx: &mut Context<'_>) {
    let items = argument_items(n);
    let count = items.len();
    if rule.allow_multiple_return_values && count > 1 {
        return;
    }
    let message = if count > 1 { format!("{MSG} {MULTI_RETURN_MSG}") } else { MSG.to_string() };
    let fix = build_fix(n, &items, ctx);
    ctx.report_with_fix(&RedundantReturn::META, n.keyword_loc().span(), message, fix);
}

/// RuboCop-AST's `ParameterizedNode::WrappedArguments#arguments`. Whitequark
/// parses a bare `return()` as zero children (same as `return` with no
/// value); Prism instead gives it one argument, an empty `ParenthesesNode`
/// (`return (nil-body)`), so that specific shape is normalized back to "no
/// arguments" to match.
fn argument_items<'pr>(n: &ReturnNode<'pr>) -> Vec<Node<'pr>> {
    let Some(args) = n.arguments() else { return Vec::new() };
    let items: Vec<Node<'pr>> = args.arguments().iter().collect();
    if let [only] = items.as_slice() {
        if only.as_parentheses_node().is_some_and(|p| p.body().is_none()) {
            return Vec::new();
        }
    }
    items
}

/// RuboCop's `correct_without_arguments`/`correct_with_arguments`.
fn build_fix(n: &ReturnNode<'_>, items: &[Node<'_>], ctx: &Context<'_>) -> Fix {
    let mut edits = Vec::new();
    if items.is_empty() {
        edits.push(Edit::replace(n.location().span(), b"nil".to_vec()));
    } else {
        if items.len() > 1 {
            // RuboCop's `add_brackets`.
            wrap(items, b"[", b"]", &mut edits);
        } else if let Some(hash) = items[0].as_keyword_hash_node() {
            // RuboCop's `hash_without_braces?` + `add_braces`: a bare
            // (braceless) implicit hash as the sole return value.
            let elements: Vec<Node<'_>> = hash.elements().iter().collect();
            wrap(&elements, b"{", b"}", &mut edits);
        }
        // RuboCop's `splat_argument?` + the `first_argument`-only strip:
        // only ever rewrites when the first return value is itself the
        // splat.
        if let Some(splat) = items.first().and_then(Node::as_splat_node) {
            if let Some(expr) = splat.expression() {
                edits.push(Edit::replace(splat.location().span(), ctx.text(expr.span()).to_vec()));
            }
        }
        let keyword = n.keyword_loc().span();
        let end = keyword_removal_end(ctx, keyword.end);
        edits.push(Edit::delete(Span::new(keyword.start, end)));
    }
    Fix { applicability: Applicability::Safe, edits }
}

/// RuboCop's `add_brackets`/`add_braces`: wraps `items` (the return's own
/// argument list, or a bare hash's own pairs) in `open`/`close` without
/// touching their own text.
fn wrap(items: &[Node<'_>], open: &'static [u8], close: &'static [u8], edits: &mut Vec<Edit>) {
    if let (Some(first), Some(last)) = (items.first(), items.last()) {
        edits.push(Edit::insert(first.span().start, open.to_vec()));
        edits.push(Edit::insert(last.span().end, close.to_vec()));
    }
}

/// RuboCop's `range_with_surrounding_space(return_node.loc.keyword, side:
/// :right)`: extends past a run of horizontal whitespace, then past a run
/// of newlines, matching `RangeHelp#final_pos`'s defaults
/// (`newlines: true, whitespace: false`).
fn keyword_removal_end(ctx: &Context<'_>, from: u32) -> u32 {
    let bytes = ctx.source().bytes();
    let mut pos = from as usize;
    while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t') {
        pos += 1;
    }
    while pos < bytes.len() && bytes[pos] == b'\n' {
        pos += 1;
    }
    u32::try_from(pos).unwrap_or(u32::MAX)
}
