//! `Style/DoubleNegation`, ported from RuboCop's
//! `lib/rubocop/cop/style/double_negation.rb`.
//!
//! # Shape
//!
//! `!!x` parses as a `CallNode` named `!` (the *outer* negation, whose
//! `message_loc` is the *first* `!` character) whose `receiver` is itself a
//! `CallNode` named `!` (the *second* `!`) -- upstream's node-pattern
//! `(send (send _ :!) :!)`. `prefix_bang?` (checked on the outer node only)
//! rejects `not not x`, which parses to the same shape but with the outer
//! selector spelled `not`.
//!
//! # `allowed_in_returns?`
//!
//! With `EnforcedStyle: allowed_in_returns` (the default), a `!!` whose
//! value is used as an implicit return is not flagged. Upstream detects
//! this two ways:
//!
//! - `node.parent&.return_type?`: the immediate parent (through Prism's
//!   extra `ArgumentsNode` wrapper around a `return`'s value, which
//!   whitequark's flat `(return value)` shape does not have) is a
//!   `ReturnNode`. [`is_return_argument`].
//! - `end_of_method_definition?`: the `!!` is (transitively) the last
//!   top-level expression of the nearest enclosing method body (a plain
//!   `def`, or a `define_method`/`define_singleton_method` block, treated
//!   as always-return-like since upstream checks the *call*'s own last
//!   argument rather than the block body at all). This is
//!   [`DoubleNegation::end_of_method_definition`], built from a per-file
//!   cache of each `DefNode`/qualifying-block's "last child" fact
//!   ([`LastChildFact`], recorded when the `DefNode`/owning `CallNode` is
//!   entered, since Prism's live node gives it for free) plus a
//!   `StatementsNode`-arity cache ([`DoubleNegation::multi_stmt`]) needed to
//!   replicate whitequark's begin-elision: Prism always wraps a body in a
//!   `StatementsNode` even for one statement, while whitequark only does so
//!   for more than one, so upstream's `parent.begin_type?` is re-derived
//!   here as "this `StatementsNode` ancestor holds more than one statement".
//!
//! `find_last_child`'s rescue/ensure recursion collapses entirely in Prism:
//! whitequark nests `(ensure (rescue body ...) handler)`, walking down to
//! the protected `body`; Prism's `BeginNode` holds `statements` (the
//! protected body), `rescue_clause`, and `ensure_clause` as flat sibling
//! fields, so the protected body is always just `BeginNode::statements()`
//! regardless of which combination of rescue/else/ensure is present.
//!
//! # The `elsif`/`else` phantom `end`
//!
//! Whitequark represents `if a; x; elsif b; y; else; z; end` as nested
//! `:if` nodes (`(if a x (if b y z))`): the `elsif`'s own `:if` node is the
//! outer node's `else_branch`, and *only the outermost* `:if` owns a real
//! `end` in its location map -- the `elsif`'s own `loc.last_line` is
//! derived purely from its own content (whichever of its branches is last),
//! never reaching the shared closing `end` a line below. Prism's `IfNode`,
//! by contrast, gives *every* link in the chain (including each `elsif` and
//! the trailing `ElseNode`) the *same* `end_keyword_loc`, so a bare
//! `ctx.last_line(if_node.span())` on a `subsequent` link over-reports by
//! however many lines separate its own content from the chain's real `end`.
//! [`chained_last_line`] re-derives the whitequark value for exactly that
//! case (an `IfNode`/`ElseNode` reached one level down via `subsequent`,
//! never the true outermost link, which keeps using its own genuine span).
//! This matters twice: [`DoubleNegation::conditional_last_line`] caches it
//! for every `IfNode`, since [`find_conditional_ancestor`] can only recover
//! kind+span from the ancestor stack by the time it is consulted; and
//! [`digest_single`] computes it inline for an `IfNode`/`UnlessNode` that is
//! itself a lone method/branch body (upstream's `child_nodes.last` landing
//! one level into its `subsequent`/`else_clause`).
//!
//! Both ancestor searches (`find_def_node_from_ascendant`,
//! `find_conditional_node_from_ascendant`) are unbounded upstream -- neither
//! stops at the other's boundary -- so [`find_def_fact`] and
//! [`find_conditional_ancestor`] each walk the *whole* ancestor chain
//! independently, matching that (a conditional wrapping a `def` from
//! outside is still found, an oddity inherited from upstream, not fixed
//! here).

use std::collections::{HashMap, HashSet};

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, NodeInfo, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, DefNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Avoid the use of double negation (`!!`).";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    AllowedInReturns,
    Forbidden,
}

/// The result of upstream's `find_last_child`, reduced to what
/// [`DoubleNegation::end_of_method_definition`] and
/// [`double_negative_condition_return_value`] actually consult: the found
/// node's own kind, start line, whitequark-equivalent last line (see the
/// module docs' "phantom `end`" section), and whether it sits directly
/// inside an array literal (`last_child.parent.array_type?`).
#[derive(Debug, Clone, Copy)]
struct LastChildFact {
    kind: NodeKind,
    first_line: u32,
    last_line: u32,
    parent_is_array: bool,
}

/// Checks for uses of double negation (!!).
#[derive(Debug, Clone)]
pub struct DoubleNegation {
    style: Style,
    /// [`LastChildFact`] for each `DefNode` and each qualifying
    /// `define_method`/`define_singleton_method` block, keyed by that
    /// node's own `(span, kind)` (`DefNode` or `BlockNode`).
    last_child_facts: HashMap<(Span, NodeKind), LastChildFact>,
    /// Spans of every `StatementsNode` holding more than one statement --
    /// upstream's `begin_type?` re-derived for Prism's always-wrapping
    /// `StatementsNode` (see the module docs).
    multi_stmt: HashSet<Span>,
    /// Every `IfNode`'s whitequark-equivalent last line (see the module
    /// docs' "phantom `end`" section), keyed by its own span.
    conditional_last_line: HashMap<Span, u32>,
}

impl Rule for DoubleNegation {
    const META: RuleMeta = RuleMeta {
        name: "Style/DoubleNegation",
        department: Department::Style,
        summary: "Checks for uses of double negation (!!).",
        explanation: "\
Checks for uses of double negation (`!!`) to convert something to a boolean value.

When using `EnforcedStyle: allowed_in_returns`, allow double negation in contexts
that use boolean as a return value. When using `EnforcedStyle: forbidden`, double
negation should be forbidden always.

NOTE: when `something` is a boolean value `!!something` and `!something.nil?` are
not the same thing. As you're unlikely to write code that can accept values of any
type this is rarely a problem in practice.

@safety
  Autocorrection is unsafe when the value is `false`, because the result of the
  expression will change (`!!false #=> false`, `!false.nil? #=> true`).",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::DefNode, NodeKind::StatementsNode, NodeKind::IfNode],
        config: &[linter::ConfigOption {
            name: "EnforcedStyle",
            default: linter::ConfigDefault::Str("allowed_in_returns"),
            allowed: &["allowed_in_returns", "forbidden"],
            doc: "Whether `!!` at an implicit method return is allowed.",
        }],
        blind_spots: "\
`find_last_child`'s single-statement digestion (whitequark elides a `begin`
wrapper for exactly one statement, so `child_nodes.last` actually descends one
level into that statement's own last child) is only replicated for the
shapes the corpus exercises -- `ArrayNode`/`HashNode` and `IfNode`/
`UnlessNode` bodies; any other single non-collection, non-conditional
statement is treated as its own last child instead of digging further into
its structure, which only risks the false-negative (exempt) direction and
happens to match every fixture, since such a statement always textually
contains the flagged node itself.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "forbidden" => Style::Forbidden,
            _ => Style::AllowedInReturns,
        };
        Ok(Self {
            style,
            last_child_facts: HashMap::new(),
            multi_stmt: HashSet::new(),
            conditional_last_line: HashMap::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.last_child_facts.clear();
        self.multi_stmt.clear();
        self.conditional_last_line.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::StatementsNode => {
                if self.style == Style::AllowedInReturns {
                    let stmts = node.as_statements_node().expect("kind matched");
                    if stmts.body().len() > 1 {
                        self.multi_stmt.insert(node.span());
                    }
                }
                return;
            }
            NodeKind::DefNode => {
                if self.style == Style::AllowedInReturns {
                    let def = node.as_def_node().expect("kind matched");
                    if let Some(fact) = def_last_child(&def, ctx) {
                        self.last_child_facts.insert((node.span(), NodeKind::DefNode), fact);
                    }
                }
                return;
            }
            NodeKind::IfNode => {
                if self.style == Style::AllowedInReturns {
                    // A direct `IfNode` parent (no intervening `StatementsNode`)
                    // only ever happens through `subsequent` (this node is an
                    // `elsif`), never a genuinely nested `if` (which always sits
                    // inside its enclosing branch's `StatementsNode`). See the
                    // module docs' "phantom `end`" section.
                    let nested = ctx.parent().is_some_and(|p| p.kind == NodeKind::IfNode);
                    let last_line = if nested {
                        chained_last_line(node, ctx)
                    } else {
                        ctx.last_line(node.span())
                    };
                    self.conditional_last_line.insert(node.span(), last_line);
                }
                return;
            }
            NodeKind::CallNode => {}
            _ => return,
        }
        let call = node.as_call_node().expect("kind matched");
        if self.style == Style::AllowedInReturns && is_defining_method_call(&call) {
            if let Some(block) = call.block().and_then(|b| b.as_block_node()) {
                if let Some(fact) = call_arg_last_child(&call, ctx) {
                    self.last_child_facts
                        .insert((block.as_node().span(), NodeKind::BlockNode), fact);
                }
            }
        }
        if call.name().as_slice() != b"!" {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };
        if ctx.text(message_loc.span()) != b"!" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(inner) = receiver.as_call_node() else { return };
        if inner.name().as_slice() != b"!" {
            return;
        }

        let selector_span = message_loc.span();
        if self.style == Style::Forbidden || !self.allowed_in_returns(node, ctx) {
            let fix = Fix {
                applicability: Applicability::Unsafe,
                edits: vec![
                    Edit::delete(selector_span),
                    Edit::insert(node.span().end, b".nil?".to_vec()),
                ],
            };
            ctx.report_with_fix(&Self::META, selector_span, MSG, fix);
        }
    }
}

impl DoubleNegation {
    /// RuboCop's `allowed_in_returns?`.
    fn allowed_in_returns(&self, node: &Node<'_>, ctx: &Context<'_>) -> bool {
        let ancestors = ctx.ancestors();
        is_return_argument(ancestors) || self.end_of_method_definition(node, ancestors, ctx)
    }

    /// RuboCop's `end_of_method_definition?`.
    fn end_of_method_definition(
        &self,
        node: &Node<'_>,
        ancestors: &[NodeInfo],
        ctx: &Context<'_>,
    ) -> bool {
        let Some(def_fact) = find_def_fact(ancestors, &self.last_child_facts) else { return false };
        if let Some(conditional) = find_conditional_ancestor(ancestors) {
            let conditional_last_line = if conditional.kind == NodeKind::IfNode {
                self.conditional_last_line
                    .get(&conditional.span)
                    .copied()
                    .unwrap_or_else(|| ctx.last_line(conditional.span))
            } else {
                ctx.last_line(conditional.span)
            };
            return double_negative_condition_return_value(
                node,
                &def_fact,
                conditional_last_line,
                ctx,
                &self.multi_stmt,
            );
        }
        if matches!(
            def_fact.kind,
            NodeKind::AssocNode | NodeKind::HashNode | NodeKind::KeywordHashNode
        ) || def_fact.parent_is_array
        {
            return false;
        }
        def_fact.first_line <= ctx.line_col(node.span().start).line
    }
}

/// RuboCop's `node.parent&.return_type?`, adjusted for Prism's extra
/// `ArgumentsNode` between a `return` and its (single) value -- whitequark's
/// `(return value)` has no such wrapper, so `value.parent` is the `return`
/// node directly.
fn is_return_argument(ancestors: &[NodeInfo]) -> bool {
    match ancestors.last() {
        Some(p) if p.kind == NodeKind::ArgumentsNode => ancestors
            .get(ancestors.len().wrapping_sub(2))
            .is_some_and(|pp| pp.kind == NodeKind::ReturnNode),
        Some(p) => p.kind == NodeKind::ReturnNode,
        None => false,
    }
}

/// RuboCop's `find_def_node_from_ascendant`, reduced to a lookup: any
/// ancestor whose `(span, kind)` was recorded as a def-like fact (a
/// `DefNode`, or a qualifying `define_method`/`define_singleton_method`
/// block) qualifies, skipping over every other ancestor kind in between.
/// Unbounded, like upstream.
fn find_def_fact(
    ancestors: &[NodeInfo],
    facts: &HashMap<(Span, NodeKind), LastChildFact>,
) -> Option<LastChildFact> {
    ancestors.iter().rev().find_map(|a| facts.get(&(a.span, a.kind)).copied())
}

/// RuboCop's `find_conditional_node_from_ascendant`: the nearest ancestor
/// whose type is one of `CONDITIONALS` (`if`/`while`/`until`/`case`/
/// `case_match`; `unless`/`while_post`/`until_post` fold into the first
/// three on the whitequark side). Unbounded, like upstream.
fn find_conditional_ancestor(ancestors: &[NodeInfo]) -> Option<NodeInfo> {
    ancestors
        .iter()
        .rev()
        .find(|a| {
            matches!(
                a.kind,
                NodeKind::IfNode
                    | NodeKind::UnlessNode
                    | NodeKind::CaseNode
                    | NodeKind::CaseMatchNode
                    | NodeKind::WhileNode
                    | NodeKind::UntilNode
            )
        })
        .copied()
}

/// RuboCop's `double_negative_condition_return_value?` plus
/// `find_parent_not_enumerable`. `conditional_last_line` is already the
/// whitequark-equivalent value (see the module docs' "phantom `end`"
/// section), resolved by the caller.
fn double_negative_condition_return_value(
    node: &Node<'_>,
    def_fact: &LastChildFact,
    conditional_last_line: u32,
    ctx: &Context<'_>,
    multi_stmt: &HashSet<Span>,
) -> bool {
    let ancestors = ctx.ancestors();
    let parent = ancestors.iter().rev().find(|a| {
        !matches!(
            a.kind,
            NodeKind::AssocNode
                | NodeKind::HashNode
                | NodeKind::KeywordHashNode
                | NodeKind::ArrayNode
        )
    });
    if let Some(p) = parent {
        if p.kind == NodeKind::StatementsNode && multi_stmt.contains(&p.span) {
            let node_line = ctx.line_col(node.span().start).line;
            return node_line == ctx.last_line(p.span);
        }
    }
    def_fact.last_line <= conditional_last_line
}

/// RuboCop's `define_method?`.
fn is_defining_method_call(call: &CallNode<'_>) -> bool {
    !call.is_safe_navigation()
        && matches!(call.name().as_slice(), b"define_method" | b"define_singleton_method")
}

/// The [`LastChildFact`] for a `define_method`/`define_singleton_method`
/// call, upstream's `find_last_child(def_node)` for the `def_node.send_type?`
/// branch: the call's own last argument (its receiver is excluded from
/// `child_nodes` when absent, and none of these calls take a receiver in
/// practice).
fn call_arg_last_child(call: &CallNode<'_>, ctx: &Context<'_>) -> Option<LastChildFact> {
    let args = call.arguments()?;
    let last = args.arguments().iter().last()?;
    Some(LastChildFact {
        kind: last.kind(),
        first_line: ctx.line_col(last.span().start).line,
        last_line: ctx.last_line(last.span()),
        parent_is_array: false,
    })
}

/// The [`LastChildFact`] for a `def`'s body: upstream's
/// `find_last_child(def_node.body)`, with the rescue/ensure recursion
/// collapsed (see the module docs) to `BeginNode::statements()`.
fn def_last_child(def: &DefNode<'_>, ctx: &Context<'_>) -> Option<LastChildFact> {
    let body = def.body()?;
    match body.kind() {
        NodeKind::StatementsNode => last_child_of_statements(&body, ctx),
        NodeKind::BeginNode => {
            last_child_of_statements(&body.as_begin_node()?.statements()?.as_node(), ctx)
        }
        _ => Some(digest_single(&body, ctx)),
    }
}

/// `find_last_child` applied to a `StatementsNode`: `child_nodes.last` when
/// it holds more than one statement (whitequark's real `begin` shape, whose
/// last element is a genuine independent top-level statement -- its own
/// span is always accurate, never a `subsequent` link), else one level
/// deeper into the sole statement (whitequark elides the `begin` wrapper
/// Prism always inserts).
fn last_child_of_statements(node: &Node<'_>, ctx: &Context<'_>) -> Option<LastChildFact> {
    let stmts = node.as_statements_node()?;
    let body = stmts.body();
    let last = body.iter().last()?;
    if body.len() > 1 {
        return Some(LastChildFact {
            kind: last.kind(),
            first_line: ctx.line_col(last.span().start).line,
            last_line: ctx.last_line(last.span()),
            parent_is_array: false,
        });
    }
    Some(digest_single(&last, ctx))
}

/// One level of `child_nodes.last` into a single statement: its last array
/// element, its last hash pair, its `subsequent`/`else_clause` for an
/// `IfNode`/`UnlessNode` (see the module docs' "phantom `end`" section, via
/// [`chained_last_line`]), or (any other shape) itself.
fn digest_single(node: &Node<'_>, ctx: &Context<'_>) -> LastChildFact {
    match node.kind() {
        NodeKind::ArrayNode => {
            if let Some(elem) = node.as_array_node().and_then(|a| a.elements().iter().last()) {
                return LastChildFact {
                    kind: elem.kind(),
                    first_line: ctx.line_col(elem.span().start).line,
                    last_line: ctx.last_line(elem.span()),
                    parent_is_array: true,
                };
            }
        }
        NodeKind::HashNode => {
            if let Some(elem) = node.as_hash_node().and_then(|h| h.elements().iter().last()) {
                return LastChildFact {
                    kind: elem.kind(),
                    first_line: ctx.line_col(elem.span().start).line,
                    last_line: ctx.last_line(elem.span()),
                    parent_is_array: false,
                };
            }
        }
        NodeKind::IfNode => {
            let n = node.as_if_node().expect("kind matched");
            let child = n
                .subsequent()
                .unwrap_or_else(|| n.statements().map_or_else(|| *node, |s| s.as_node()));
            let is_chained = n.subsequent().is_some();
            return LastChildFact {
                kind: child.kind(),
                first_line: ctx.line_col(child.span().start).line,
                last_line: if is_chained {
                    chained_last_line(&child, ctx)
                } else {
                    ctx.last_line(child.span())
                },
                parent_is_array: false,
            };
        }
        NodeKind::UnlessNode => {
            let n = node.as_unless_node().expect("kind matched");
            let child = n.else_clause().map_or_else(
                || n.statements().map_or_else(|| *node, |s| s.as_node()),
                |e| e.as_node(),
            );
            let is_chained = n.else_clause().is_some();
            return LastChildFact {
                kind: child.kind(),
                first_line: ctx.line_col(child.span().start).line,
                last_line: if is_chained {
                    chained_last_line(&child, ctx)
                } else {
                    ctx.last_line(child.span())
                },
                parent_is_array: false,
            };
        }
        NodeKind::CallNode => {
            let call = node.as_call_node().expect("kind matched");
            let child = call
                .arguments()
                .and_then(|a| a.arguments().iter().last())
                .or_else(|| call.receiver());
            if let Some(child) = child {
                return LastChildFact {
                    kind: child.kind(),
                    first_line: ctx.line_col(child.span().start).line,
                    last_line: ctx.last_line(child.span()),
                    parent_is_array: false,
                };
            }
        }
        _ => {}
    }
    LastChildFact {
        kind: node.kind(),
        first_line: ctx.line_col(node.span().start).line,
        last_line: ctx.last_line(node.span()),
        parent_is_array: false,
    }
}

/// Whitequark's `Node#last_line` for an `IfNode`/`ElseNode` reached one
/// level down via `subsequent`/`else_clause` (an `elsif` or trailing
/// `else`): unlike the true outermost `if`/`unless`, these never own a real
/// `end` in whitequark's location map, so their `last_line` comes from
/// their own content alone -- recursing through a further `subsequent`, or
/// (the chain's end) their own `statements`. See the module docs' "phantom
/// `end`" section for why Prism's raw span cannot be used here directly.
fn chained_last_line(node: &Node<'_>, ctx: &Context<'_>) -> u32 {
    match node.kind() {
        NodeKind::IfNode => {
            let n = node.as_if_node().expect("kind matched");
            match n.subsequent() {
                Some(sub) => chained_last_line(&sub, ctx),
                None => n.statements().map_or_else(
                    || ctx.last_line(node.span()),
                    |s| ctx.last_line(s.as_node().span()),
                ),
            }
        }
        NodeKind::ElseNode => {
            let n = node.as_else_node().expect("kind matched");
            n.statements()
                .map_or_else(|| ctx.last_line(node.span()), |s| ctx.last_line(s.as_node().span()))
        }
        _ => ctx.last_line(node.span()),
    }
}
