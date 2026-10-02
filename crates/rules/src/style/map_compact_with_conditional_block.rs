//! `Style/MapCompactWithConditionalBlock`, ported from RuboCop's
//! `lib/rubocop/cop/style/map_compact_with_conditional_block.rb`.
//!
//! No `VariableForce`/parent-map is needed: every piece this cop inspects
//! (the `map`/`filter_map` call, its attached block, the conditional inside,
//! the block parameter) is reachable from the `compact`/`filter_map` call
//! node itself through plain Prism accessors. The one exception is the
//! "`arr.filter_map { ... }.compact`" double match RuboCop's own node
//! pattern allows (both the bare `filter_map` call *and* the `compact` call
//! that follows it match the same block; RuboCop's `add_offense` silently
//! drops the second, overlapping one via its per-round `Set` of offense
//! ranges). Here that is replicated with a `handled` set of already-claimed
//! `map`/`filter_map` call spans, populated when the `compact` path (visited
//! first, since `enter` runs in pre-order and `compact`'s receiver is the
//! `map`/`filter_map` call) claims one.
//!
//! whitequark's `unless` parses into the *same* `:if` node type with its
//! `then`/`else` children swapped (`Parser::CurrentRuby.parse("unless c; a;
//! else; b; end")` is `s(:if, c, b, a)`); Prism's separate `UnlessNode`
//! keeps the written order (`statements` is the unless-body, `else_clause`
//! the explicit else). `if_branches` below restates that swap so pattern
//! matching sees the same raw shape upstream's `def_node_matcher` pattern
//! does, and so that "truthy content sits in the raw `then` slot" holds
//! uniformly for `if`, ternary, and `unless` alike (confirmed against
//! `rubocop-ast`'s `IfNode#if_branch`/`#else_branch`, which reduce to the
//! same raw-slot comparison for both keywords once `node_parts` is
//! unfolded). Only the two-statement guard-clause shape (`next if cond \n
//! item`) needs its own `if`-vs-`unless` branch, mirroring upstream's
//! `truthy_branch_for_guard?`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, CallNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use std::collections::HashSet;

/// RuboCop's `MSG`.
const MSG_FMT: (&str, &str) = ("Replace `", "` with `");

/// Prefer `select` or `reject` over `map { ... }.compact`.
#[derive(Debug, Clone, Default)]
pub struct MapCompactWithConditionalBlock {
    /// Spans of `map`/`filter_map` calls already claimed by the `compact` path, so the bare
    /// `filter_map` path (visited later, since it is the `compact` call's receiver) does not
    /// register a second, overlapping offense for the same block.
    handled: HashSet<Span>,
}

impl Rule for MapCompactWithConditionalBlock {
    const META: RuleMeta = RuleMeta {
        name: "Style/MapCompactWithConditionalBlock",
        department: Department::Style,
        summary: "Prefer `select` or `reject` over `map { ... }.compact`.",
        explanation: "\
This cop also handles `filter_map { ... }`, similar to `map { ... }.compact`.

@safety
  This cop is unsafe because `compact` also removes `nil` elements that were already present in \
the receiver, whereas `select`/`reject` keep them. The result therefore differs when the \
collection contains `nil`:

  [nil, 1].map { |e| e if e }.compact # => [1]
  [nil, 1].select { |e| e }           # => [nil, 1]",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        match call.name().as_slice() {
            b"compact" => self.inspect_compact(&call, ctx),
            b"filter_map" => self.inspect_bare_filter_map(&call, ctx),
            _ => {}
        }
    }
}

impl MapCompactWithConditionalBlock {
    /// RuboCop's `on_send` when `map_candidate = node.children.first` matches: `node` is the
    /// `.compact` call, `map_candidate` its receiver.
    fn inspect_compact(&mut self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        if call.arguments().is_some() {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(map_call) = receiver.as_call_node() else { return };
        if !matches!(map_call.name().as_slice(), b"map" | b"filter_map") {
            return;
        }
        let Some(block) = map_call.block().and_then(|b| b.as_block_node()) else { return };
        let Some(matched) = match_block_shape(&block) else { return };
        if !returns_block_argument(&matched) {
            return;
        }

        let method = if matched.truthy { "select" } else { "reject" };
        let current =
            format!("{} {{ ... }}.compact", String::from_utf8_lossy(map_call.name().as_slice()));
        let message = format!("{}{current}{}{method}`.", MSG_FMT.0, MSG_FMT.1);
        let Some(selector) = map_call.message_loc() else { return };
        let range = Span::new(selector.span().start, call.as_node().span().end);

        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            replacement_fix(ctx, range, method, &matched),
        );
        self.handled.insert(map_call.as_node().span());
    }

    /// RuboCop's `on_send` when `map_candidate = conditional_block(node.parent)` matches: `node`
    /// is the bare `filter_map` call (no `.compact` chained), its own attached block.
    fn inspect_bare_filter_map(&mut self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        if self.handled.contains(&call.as_node().span()) {
            return;
        }
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        let Some(matched) = match_block_shape(&block) else { return };
        if !returns_block_argument(&matched) {
            return;
        }

        let method = if matched.truthy { "select" } else { "reject" };
        let message = format!("{}filter_map {{ ... }}{}{method}`.", MSG_FMT.0, MSG_FMT.1);
        let Some(selector) = call.message_loc() else { return };
        let range = Span::new(selector.span().start, block.as_node().span().end);

        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            replacement_fix(ctx, range, method, &matched),
        );
    }
}

/// One successful `conditional_block` match.
struct Matched<'pr> {
    block_argument: &'pr [u8],
    condition: Node<'pr>,
    return_value: Node<'pr>,
    truthy: bool,
}

/// RuboCop's `returns_block_argument?`.
fn returns_block_argument(matched: &Matched<'_>) -> bool {
    matched
        .return_value
        .as_local_variable_read_node()
        .is_some_and(|lvar| lvar.name().as_slice() == matched.block_argument)
}

/// Builds the `method { |arg| condition }` replacement, preserving the condition's exact source
/// text (RuboCop's `condition_node.source`).
fn replacement_fix(ctx: &Context<'_>, range: Span, method: &str, matched: &Matched<'_>) -> Fix {
    let mut text = Vec::new();
    text.extend_from_slice(method.as_bytes());
    text.extend_from_slice(b" { |");
    text.extend_from_slice(matched.block_argument);
    text.extend_from_slice(b"| ");
    text.extend_from_slice(ctx.text(matched.condition.span()));
    text.extend_from_slice(b" }");
    Fix { applicability: Applicability::Unsafe, edits: vec![Edit::replace(range, text)] }
}

/// RuboCop's `conditional_block` node pattern.
fn match_block_shape<'pr>(block: &BlockNode<'pr>) -> Option<Matched<'pr>> {
    let block_argument = single_required_param_name(block)?;
    let body = block.body()?;
    let stmts = body.as_statements_node()?;
    let items: Vec<Node<'pr>> = stmts.body().iter().collect();
    let (condition, return_value, truthy) = match items.as_slice() {
        [only] => {
            let (condition, then_raw, else_raw) = if_branches(only)?;
            let (return_value, truthy) = match_simple(then_raw, else_raw)?;
            (condition, return_value, truthy)
        }
        [first, second] => {
            let (condition, then_raw, else_raw) = if_branches(first)?;
            let is_if = first.as_if_node().is_some();
            let (return_value, truthy) = match_begin(then_raw, else_raw, *second, is_if)?;
            (condition, return_value, truthy)
        }
        _ => return None,
    };
    Some(Matched { block_argument, condition, return_value, truthy })
}

/// RuboCop-AST's `(args (arg $_))` shape check, returning the parameter's name.
fn single_required_param_name<'pr>(block: &BlockNode<'pr>) -> Option<&'pr [u8]> {
    let params = block.parameters()?.as_block_parameters_node()?;
    if !params.locals().is_empty() {
        return None;
    }
    let inner = params.parameters()?;
    if inner.requireds().len() != 1
        || !inner.optionals().is_empty()
        || inner.rest().is_some()
        || !inner.posts().is_empty()
        || !inner.keywords().is_empty()
        || inner.keyword_rest().is_some()
        || inner.block().is_some()
    {
        return None;
    }
    let req = inner.requireds().iter().next()?.as_required_parameter_node()?;
    Some(req.name().as_slice())
}

/// The sole statement of a branch body, RuboCop's raw (pre-`single-statement-begin-elision`)
/// child: `None` for zero statements (matches whitequark's literal absence); a multi-statement
/// body is handed back as its own (non-matching) `StatementsNode`, never collapsed to `None`.
fn branch_value<'pr>(stmts: Option<ruby_ast::node::StatementsNode<'pr>>) -> Option<Node<'pr>> {
    let stmts = stmts?;
    let items: Vec<Node<'pr>> = stmts.body().iter().collect();
    match items.as_slice() {
        [] => None,
        [only] => Some(*only),
        _ => Some(stmts.as_node()),
    }
}

/// Restates whitequark's raw `(if cond then else)` shape for both `IfNode` (ternary included) and
/// `UnlessNode` (whose written body/explicit-else map to the *swapped* raw `else`/`then`
/// positions -- see the module doc comment).
fn if_branches<'pr>(node: &Node<'pr>) -> Option<(Node<'pr>, Option<Node<'pr>>, Option<Node<'pr>>)> {
    if let Some(if_node) = node.as_if_node() {
        let then_raw = branch_value(if_node.statements());
        let else_raw = match if_node.subsequent() {
            None => None,
            Some(sub) => branch_value(sub.as_else_node()?.statements()),
        };
        Some((if_node.predicate(), then_raw, else_raw))
    } else if let Some(unless_node) = node.as_unless_node() {
        let then_raw = unless_node.else_clause().and_then(|e| branch_value(e.statements()));
        let else_raw = branch_value(unless_node.statements());
        Some((unless_node.predicate(), then_raw, else_raw))
    } else {
        None
    }
}

fn is_absent(n: Option<Node<'_>>) -> bool {
    n.is_none()
}

fn is_nil_literal(n: Option<Node<'_>>) -> bool {
    n.is_some_and(|x| x.kind() == NodeKind::NilNode)
}

/// RuboCop's bareword `next` pattern token: any `next` node, regardless of its own arguments.
fn is_any_next(n: Option<Node<'_>>) -> bool {
    n.is_some_and(|x| x.kind() == NodeKind::NextNode)
}

/// RuboCop's `(next $(lvar _))`: a `next` with exactly one local-variable-read argument.
fn next_lvar<'pr>(n: Option<Node<'pr>>) -> Option<Node<'pr>> {
    let next = n?.as_next_node()?;
    let args = next.arguments()?;
    let list: Vec<Node<'pr>> = args.arguments().iter().collect();
    match list.as_slice() {
        [only] if only.as_local_variable_read_node().is_some() => Some(*only),
        _ => None,
    }
}

fn as_lvar(n: Option<Node<'_>>) -> Option<Node<'_>> {
    n.filter(|x| x.kind() == NodeKind::LocalVariableReadNode)
}

fn next_has_args(n: Option<Node<'_>>) -> bool {
    n.and_then(|x| x.as_next_node()).is_some_and(|next| next.arguments().is_some())
}

/// Patterns 1-4: a single-statement `if`/`unless`/ternary body. Returns the captured value and
/// whether it sits in the raw "then" slot (RuboCop's `truthy_branch_for_if?`, which reduces to
/// this same raw-slot comparison for `if`, ternary, and `unless` alike).
fn match_simple<'pr>(
    then_raw: Option<Node<'pr>>,
    else_raw: Option<Node<'pr>>,
) -> Option<(Node<'pr>, bool)> {
    if let Some(r) = as_lvar(then_raw) {
        if is_any_next(else_raw) || is_absent(else_raw) {
            return Some((r, true));
        }
    }
    if let Some(r) = as_lvar(else_raw) {
        if is_any_next(then_raw) || is_absent(then_raw) {
            return Some((r, false));
        }
    }
    if let Some(r) = next_lvar(then_raw) {
        if is_any_next(else_raw) || is_nil_literal(else_raw) || is_absent(else_raw) {
            return Some((r, true));
        }
    }
    if let Some(r) = next_lvar(else_raw) {
        if is_any_next(then_raw) || is_nil_literal(then_raw) || is_absent(then_raw) {
            return Some((r, false));
        }
    }
    None
}

/// Patterns 5-6: a two-statement `begin` body (a guard clause followed by the return value, or a
/// `next`-with-value guard followed by a trailing literal `nil`).
fn match_begin<'pr>(
    then_raw: Option<Node<'pr>>,
    else_raw: Option<Node<'pr>>,
    second: Node<'pr>,
    is_if: bool,
) -> Option<(Node<'pr>, bool)> {
    if let Some(r) = as_lvar(Some(second)) {
        // RuboCop's `truthy_branch_for_guard?`.
        let guard = if is_any_next(then_raw) && is_absent(else_raw) {
            then_raw
        } else if is_absent(then_raw) && is_any_next(else_raw) {
            else_raw
        } else {
            return None;
        };
        let has_args = next_has_args(guard);
        let truthy = if is_if { has_args } else { !has_args };
        return Some((r, truthy));
    }
    if is_nil_literal(Some(second)) {
        if let Some(r) = next_lvar(then_raw) {
            if is_absent(else_raw) {
                return Some((r, true));
            }
        }
        if let Some(r) = next_lvar(else_raw) {
            if is_absent(then_raw) {
                return Some((r, false));
            }
        }
    }
    None
}
