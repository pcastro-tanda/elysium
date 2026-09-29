//! `Style/IfWithSemicolon`, ported from RuboCop's
//! `lib/rubocop/cop/style/if_with_semicolon.rb` plus the `OnNormalIfUnless`
//! mixin it includes.
//!
//! # Node shapes
//!
//! `if`/`elsif` are both Prism's [`IfNode`]; an `elsif` link is reached only
//! through [`IfNode::subsequent`] with no wrapping node in between, so its
//! immediate parent on the traversal's ancestor stack is always the
//! enclosing `IfNode` -- the same signal upstream's `node.parent&.if_type?`
//! uses to skip visiting it as an independent statement. `unless` is always
//! Prism's own [`UnlessNode`] and never chains (`else_clause` only, no
//! `elsif`). A ternary has `if_keyword_loc` `None`; a modifier form has
//! `end_keyword_loc` `None` -- both are skipped, matching
//! `OnNormalIfUnless#on_if`.
//!
//! Whitequark's single `if`-node shape normalizes `unless` so that
//! `if_branch`/`else_branch` read positionally regardless of keyword (the
//! branch right after the condition is always `if_branch`, the one after
//! `else` is always `else_branch`); Prism's `statements()`/`else_clause()`
//! already read that way natively, so the only place the `if`/`unless`
//! distinction still matters is the final ternary assembly, where
//! `replacement` swaps the two computed expressions for `unless`.
//!
//! # `ignore_node`
//!
//! Upstream calls `ignore_node(node)` right after every `add_offense`, and
//! `part_of_ignored_node?` makes any node nested inside an already-reported
//! one invisible to this cop for the rest of the investigation -- including
//! skipping the offense entirely (unlike e.g. `Style/UnlessElse`, which
//! still reports but withholds the fix). This rule is cloned per file and
//! visited top-down, so an outer offense is always recorded before its
//! nested descendants are visited; a plain `Vec<Span>` of already-reported
//! spans reproduces the same order-dependent behaviour.
//!
//! # The bare-`if`-in-`else` quirk
//!
//! Whitequark represents a real `elsif` and a lone `if` statement written
//! as the sole expression of an `else` clause (`else if x; y end end`, no
//! `elsif` keyword) identically: both splice the nested `if` node directly
//! into the `else_branch` position. Prism tells them apart structurally
//! (`subsequent` is the `IfNode` directly for a real `elsif`, vs. an
//! `ElseNode` wrapping a `StatementsNode` containing the `IfNode` for the
//! bare case), but this port intentionally erases that distinction wherever
//! upstream's `else_branch.if_type?` check would have: [`effective_tail`]
//! and [`build_else_branch`] both treat a lone `if` statement found as an
//! `else` clause's only statement the same as a genuine `elsif` continuing
//! the chain.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, ElseNode, IfNode, StatementsNode, UnlessNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use std::fmt::Write as _;

/// `MSG_IF_ELSE`/`MSG_NEWLINE`/`MSG_TERNARY`, merged into one formatter
/// since they only ever differ in their trailing clause.
fn message(keyword: &str, condition_text: &str, use_newline: bool, use_if_else: bool) -> String {
    let suffix = if use_newline {
        "use a newline instead."
    } else if use_if_else {
        "use `if/else` instead."
    } else {
        "use a ternary operator instead."
    };
    format!("Do not use `{keyword} {condition_text};` - {suffix}")
}

/// What follows the primary branch of an `if`/`unless`/`elsif` node --
/// RuboCop-AST's `IfNode#else_branch`/`#branches` machinery, restated over
/// Prism's `subsequent`/`else_clause` fields.
enum Tail<'pr> {
    /// No `elsif`/`else` at all.
    None,
    /// A real `elsif` continuation.
    Elsif(IfNode<'pr>),
    /// A terminal `else` clause.
    Else(ElseNode<'pr>),
}

/// `IfNode::subsequent`, classified.
fn if_tail<'pr>(node: &IfNode<'pr>) -> Tail<'pr> {
    match node.subsequent() {
        None => Tail::None,
        Some(inner) => match inner.as_if_node() {
            Some(elsif) => Tail::Elsif(elsif),
            None => Tail::Else(inner.as_else_node().expect("`if` subsequent is always if or else")),
        },
    }
}

/// `UnlessNode::else_clause`, classified (never an `elsif`: `unless` cannot
/// chain).
fn unless_tail<'pr>(node: &UnlessNode<'pr>) -> Tail<'pr> {
    match node.else_clause() {
        None => Tail::None,
        Some(e) => Tail::Else(e),
    }
}

/// A `StatementsNode` with exactly one statement, standing in for that
/// statement itself -- whitequark elides the `begin` wrapper for a single
/// statement, Prism never does.
fn single_statement<'pr>(stmts: &StatementsNode<'pr>) -> Option<Node<'pr>> {
    let body = stmts.body();
    if body.len() == 1 {
        body.iter().next()
    } else {
        None
    }
}

/// Calls `f` once per branch body across the whole `if`/`elsif*`/`else`
/// chain, in source order -- RuboCop-AST's `IfNode#branches`, uncompacted
/// (callers decide what an absent branch means for their own check).
fn for_each_branch<'pr>(
    first: Option<StatementsNode<'pr>>,
    tail: &Tail<'pr>,
    f: &mut impl FnMut(Option<StatementsNode<'pr>>),
) {
    f(first);
    match tail {
        Tail::None => {}
        Tail::Elsif(inner) => for_each_branch(inner.statements(), &if_tail(inner), f),
        Tail::Else(e) => f(e.statements()),
    }
}

/// RuboCop-AST's `Node::ASSIGNMENTS`, restated over Prism's per-target-kind
/// `*WriteNode` family (see `Style/CaseEquality`'s identical copy).
fn is_assignment(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::LocalVariableWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::MultiWriteNode
            | NodeKind::CallOperatorWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::CallOrWriteNode
            | NodeKind::IndexOperatorWriteNode
            | NodeKind::IndexAndWriteNode
            | NodeKind::IndexOrWriteNode
    )
}

/// RuboCop's `require_newline?`: some branch spans two or more statements
/// (whitequark's `begin_type?`, here "more than one statement in its
/// `StatementsNode`"), or is a bare `return` carrying a value.
fn requires_newline(first: Option<StatementsNode<'_>>, tail: &Tail<'_>) -> bool {
    let mut found = false;
    for_each_branch(first, tail, &mut |stmts| {
        if found {
            return;
        }
        let Some(stmts) = stmts else { return };
        if stmts.body().len() > 1 {
            found = true;
            return;
        }
        if let Some(one) = single_statement(&stmts) {
            if let Some(ret) = one.as_return_node() {
                if ret.arguments().is_some() {
                    found = true;
                }
            }
        }
    });
    found
}

/// RuboCop's `use_masgn_or_block_in_branches?`: some branch is a single
/// multiple-assignment statement, or a single call carrying a literal block
/// (not a `&blk` pass-through argument -- `CallNode::block` yields both, so
/// [`Node::as_block_node`] filters to the literal-block shapes only).
fn uses_masgn_or_block(first: Option<StatementsNode<'_>>, tail: &Tail<'_>) -> bool {
    let mut found = false;
    for_each_branch(first, tail, &mut |stmts| {
        if found {
            return;
        }
        let Some(one) = stmts.and_then(|s| single_statement(&s)) else { return };
        if one.as_multi_write_node().is_some() {
            found = true;
            return;
        }
        if let Some(call) = one.as_call_node() {
            if call.block().and_then(|b| b.as_block_node()).is_some() {
                found = true;
            }
        }
    });
    found
}

/// RuboCop's `node.else_branch` read on the outer node itself: the single
/// statement continuing the chain, whether a genuine `elsif` or (per the
/// module doc) a lone `if` statement standing alone in an `else` clause.
fn effective_tail<'pr>(tail: &Tail<'pr>) -> Option<Node<'pr>> {
    match tail {
        Tail::None => None,
        Tail::Elsif(inner) => Some(inner.as_node()),
        Tail::Else(e) => e.statements().and_then(|s| single_statement(&s)),
    }
}

/// RuboCop's `ARITHMETIC_OPERATORS`.
fn is_arithmetic_operator(name: &[u8]) -> bool {
    matches!(name, b"+" | b"-" | b"*" | b"/" | b"%" | b"**")
}

/// RuboCop's `require_argument_parentheses?`: an unparenthesized call
/// (`send`/`csend` both count, per `call_type?`) with at least one
/// argument, that is neither an arithmetic operator nor `[]`/`[]=`.
fn requires_argument_parens<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    let name = call.name();
    let name = name.as_slice();
    if is_arithmetic_operator(name) || name == b"[]" || name == b"[]=" {
        return None;
    }
    if call.opening_loc().is_some() {
        return None;
    }
    let args = call.arguments()?;
    if args.arguments().is_empty() {
        return None;
    }
    Some(call)
}

/// RuboCop's `build_expression`: `nil` for an absent branch, the call
/// re-wrapped with explicit parentheses around its arguments when
/// [`requires_argument_parens`] applies, otherwise the branch's own source
/// verbatim (may itself span several lines).
fn build_expression(ctx: &Context<'_>, node: Option<Node<'_>>) -> String {
    let Some(node) = node else { return "nil".to_string() };
    if let Some(call) = requires_argument_parens(&node) {
        let message_end = call.message_loc().expect("call has a method name").span().end;
        let method_text = ctx.text(Span::new(node.span().start, message_end));
        let args = call.arguments().expect("checked Some above");
        let first_arg = args.arguments().iter().next().expect("checked non-empty above");
        let args_text = ctx.text(Span::new(first_arg.span().start, node.span().end));
        format!("{}({})", String::from_utf8_lossy(method_text), String::from_utf8_lossy(args_text))
    } else {
        String::from_utf8_lossy(ctx.text(node.span())).into_owned()
    }
}

/// RuboCop's `ternary_condition`: an assignment condition must be
/// parenthesized so the ternary does not capture it (`a = b ? c : d` would
/// assign the whole ternary to `a`, not just `b`).
fn ternary_condition_text(ctx: &Context<'_>, predicate: &Node<'_>) -> String {
    let text = String::from_utf8_lossy(ctx.text(predicate.span())).into_owned();
    if is_assignment(predicate) {
        format!("({text})")
    } else {
        text
    }
}

/// RuboCop's `build_else_branch`, called on an `elsif` continuation (or the
/// bare-`if`-in-`else` stand-in for one, per the module doc): renders it and
/// recurses into whatever continues the chain.
fn build_else_branch(ctx: &Context<'_>, link: &IfNode<'_>) -> String {
    let cond_text = String::from_utf8_lossy(ctx.text(link.predicate().span())).into_owned();
    let then_text = link
        .statements()
        .and_then(|s| single_statement(&s))
        .map(|n| String::from_utf8_lossy(ctx.text(n.span())).into_owned())
        .unwrap_or_default();
    let mut result = format!("elsif {cond_text}\n  {then_text}\n");
    match if_tail(link) {
        Tail::None => {}
        Tail::Elsif(inner) => result.push_str(&build_else_branch(ctx, &inner)),
        Tail::Else(e) => {
            if let Some(stmt) = e.statements().and_then(|s| single_statement(&s)) {
                if let Some(inner_if) = stmt.as_if_node() {
                    result.push_str(&build_else_branch(ctx, &inner_if));
                } else {
                    let text = String::from_utf8_lossy(ctx.text(stmt.span())).into_owned();
                    let _ = write!(result, "else\n  {text}\n");
                }
            }
        }
    }
    result
}

/// RuboCop's `correct_elsif`: rebuilds the whole chain as a multi-line
/// `if`/`elsif`/`else`, since a ternary cannot express more than two
/// branches.
fn build_correct_elsif(
    ctx: &Context<'_>,
    predicate: &Node<'_>,
    first: Option<StatementsNode<'_>>,
    elsif_link: &IfNode<'_>,
) -> String {
    let cond_text = String::from_utf8_lossy(ctx.text(predicate.span())).into_owned();
    let then_text = first
        .and_then(|s| single_statement(&s))
        .map(|n| String::from_utf8_lossy(ctx.text(n.span())).into_owned())
        .unwrap_or_default();
    let mut body = build_else_branch(ctx, elsif_link);
    if body.ends_with('\n') {
        body.pop();
    }
    format!("if {cond_text}\n  {then_text}\n{body}\nend")
}

/// RuboCop's `replacement`: a ternary rewrite of the whole `if`/`unless`,
/// swapping the two branches for `unless` (whose primary branch is the
/// falsy one).
fn build_ternary(
    ctx: &Context<'_>,
    is_unless: bool,
    predicate: &Node<'_>,
    first: Option<StatementsNode<'_>>,
    tail: &Tail<'_>,
) -> String {
    let then_stmt = first.and_then(|s| single_statement(&s));
    let else_stmt = match tail {
        Tail::Else(e) => e.statements().and_then(|s| single_statement(&s)),
        _ => None,
    };
    let mut then_code = build_expression(ctx, then_stmt);
    let mut else_code = build_expression(ctx, else_stmt);
    if is_unless {
        std::mem::swap(&mut then_code, &mut else_code);
    }
    let condition_text = ternary_condition_text(ctx, predicate);
    format!("{condition_text} ? {then_code} : {else_code}")
}

/// RuboCop's `replacement`, dispatching to [`build_correct_elsif`] when the
/// chain continues.
fn build_replacement(
    ctx: &Context<'_>,
    is_unless: bool,
    predicate: &Node<'_>,
    first: Option<StatementsNode<'_>>,
    tail: &Tail<'_>,
) -> String {
    match effective_tail(tail) {
        Some(n) if n.as_if_node().is_some() => {
            let elsif_link = n.as_if_node().expect("checked Some above");
            build_correct_elsif(ctx, predicate, first, &elsif_link)
        }
        _ => build_ternary(ctx, is_unless, predicate, first, tail),
    }
}

/// The `;` token ending the condition, if that is literally what separates
/// it from the body -- RuboCop's `node.loc.begin&.is?(';')`. Prism folds
/// both a `;` and an implicit newline separator into `then_keyword_loc:
/// None`, so this scans the raw source instead: skip spaces/tabs right
/// after the condition (never newlines, which would mean no `;` is
/// present) and check the next byte.
fn find_semicolon(ctx: &Context<'_>, predicate_end: u32, search_end: u32) -> Option<Span> {
    let bytes = ctx.text(Span::new(predicate_end, search_end));
    let mut i = 0usize;
    while matches!(bytes.get(i), Some(b' ' | b'\t')) {
        i += 1;
    }
    if bytes.get(i) == Some(&b';') {
        let pos = predicate_end + u32::try_from(i).unwrap_or(0);
        Some(Span::new(pos, pos + 1))
    } else {
        None
    }
}

/// Bundles [`IfWithSemicolon::check`]'s inputs (grouped to stay under
/// `clippy::too_many_arguments`).
struct IfShape<'pr> {
    node_span: Span,
    keyword: &'static str,
    is_unless: bool,
    predicate: Node<'pr>,
    first: Option<StatementsNode<'pr>>,
    tail: Tail<'pr>,
}

/// Do not use if x; .... Use the ternary operator instead.
#[derive(Debug, Clone, Default)]
pub struct IfWithSemicolon {
    /// Spans of nodes already reported; a node nested inside one of these
    /// is `part_of_ignored_node?` and is skipped entirely (no offense, no
    /// fix) -- see the module doc.
    ignored: Vec<Span>,
}

impl IfWithSemicolon {
    /// RuboCop's `on_normal_if_unless`.
    fn check(&mut self, ctx: &mut Context<'_>, shape: IfShape<'_>) {
        let IfShape { node_span, keyword, is_unless, predicate, first, tail } = shape;
        let part_of_ignored =
            self.ignored.iter().any(|s| s.start <= node_span.start && node_span.end <= s.end);
        if part_of_ignored {
            return;
        }

        let Some(semicolon_span) = find_semicolon(ctx, predicate.span().end, node_span.end) else {
            return;
        };

        let use_newline = requires_newline(first, &tail);
        let use_masgn_or_block = uses_masgn_or_block(first, &tail);
        let use_if_else =
            effective_tail(&tail).is_some_and(|n| n.as_if_node().is_some()) || use_masgn_or_block;

        let condition_text = String::from_utf8_lossy(ctx.text(predicate.span())).into_owned();
        let msg = message(keyword, &condition_text, use_newline, use_if_else);

        let (edit_span, replacement) = if use_newline || use_masgn_or_block {
            (semicolon_span, b"\n".to_vec())
        } else {
            (node_span, build_replacement(ctx, is_unless, &predicate, first, &tail).into_bytes())
        };

        ctx.report_with_fix(
            &<Self as Rule>::META,
            node_span,
            msg,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(edit_span, replacement)],
            },
        );

        self.ignored.push(node_span);
    }
}

impl Rule for IfWithSemicolon {
    const META: RuleMeta = RuleMeta {
        name: "Style/IfWithSemicolon",
        department: Department::Style,
        summary: "Do not use if x; .... Use the ternary operator instead.",
        explanation: "Checks for uses of semicolon in if statements.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::IfNode { .. } => {
                let if_node = node.as_if_node().expect("kind matched");
                if if_node.if_keyword_loc().is_none() {
                    return; // ternary
                }
                if if_node.end_keyword_loc().is_none() {
                    return; // modifier form
                }
                if ctx.parent().is_some_and(|p| p.kind == NodeKind::IfNode) {
                    return; // reached only via a parent `IfNode::subsequent`: this is an `elsif` link
                }
                let tail = if_tail(&if_node);
                self.check(
                    ctx,
                    IfShape {
                        node_span: if_node.location().span(),
                        keyword: "if",
                        is_unless: false,
                        predicate: if_node.predicate(),
                        first: if_node.statements(),
                        tail,
                    },
                );
            }
            Node::UnlessNode { .. } => {
                let unless_node = node.as_unless_node().expect("kind matched");
                if unless_node.end_keyword_loc().is_none() {
                    return; // modifier form
                }
                let tail = unless_tail(&unless_node);
                self.check(
                    ctx,
                    IfShape {
                        node_span: unless_node.location().span(),
                        keyword: "unless",
                        is_unless: true,
                        predicate: unless_node.predicate(),
                        first: unless_node.statements(),
                        tail,
                    },
                );
            }
            _ => {}
        }
    }
}
