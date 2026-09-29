//! `Lint/LiteralAsCondition`, ported from RuboCop's
//! `lib/rubocop/cop/lint/literal_as_condition.rb`.
//!
//! # Callback shape
//!
//! Upstream's `on_and`/`on_or`/`on_send` fire for *every* `and`/`or`/`!`
//! (`RESTRICT_ON_SEND = [:!]`) node in the file, not just ones that sit in
//! boolean/condition position -- e.g. `case a || 1` never reaches
//! `check_case` (its `condition` is an `OrNode`, not itself a literal), but
//! the very same `1 && a` nested inside `!(1 && a)` gets its own
//! independent `on_and` visit regardless of where it sits. This port
//! mirrors that by dispatching on node *kind* alone, exactly like
//! upstream's callback registration.
//!
//! # whitequark -> Prism node-range traps
//!
//! whitequark's parser gives a nested `elsif` link no `loc.end` at all (the
//! literal `end` token belongs solely to the chain's head); that link's own
//! `source_range` is simply the union of its own children's ranges, which
//! happens to stop right before any following `elsif`/`else` link's own
//! text -- or, if this link itself carries its own trailing `else`, extends
//! through that `else` body, but *never* through the terminal `end` (see
//! `Lint/EmptyConditionalBody`'s module doc for the same distinction).
//! Prism instead gives *every* link of the chain (head and every `elsif`)
//! the identical, shared `end_keyword_loc`, and computes each nested
//! `IfNode`'s own `span()` as running all the way through that shared
//! `end` too. [`if_own_span`] reconstructs the whitequark-shaped range by
//! hand: a head node's (`if_keyword_loc` reading `"if"`) own span always
//! runs through the real, shared terminal `end`; an `elsif` link's own span
//! stops at the end of its own last content (its own trailing `else`
//! clause if it has one, recursing through a further nested `elsif`
//! otherwise, or just its own body if it has neither), deliberately
//! excluding that terminal `end`.
//!
//! `range_with_comments(node.if_branch)` (used only when converting a
//! *truthy* `elsif` into a plain `else`) is reconstructed as
//! [`extend_through_trailing_comment`]: a same-line comment immediately
//! after that branch's own text needs to be folded into the replacement,
//! since [`if_own_span`] for that `elsif` link extends past it (through a
//! further `else`/`elsif`) -- whereas every other branch of the ladder
//! either has no further chain past its own replaced text (so a trailing
//! comment sits *outside* the replaced span and survives untouched for
//! free) or upstream itself never bothers to preserve it.
//!
//! # `begin...end while`/`until` (post-condition loops)
//!
//! Prism represents the do-while body as a `StatementsNode` wrapping a
//! single [`ruby_ast::node::BeginNode`] (the literal `begin...end` block),
//! not the bare statement list directly -- whitequark's `(kwbegin ...)`
//! node in the same `body` slot. [`unroll_body`] unwraps that one level to
//! reach the real statements before joining their sources with `"\n"`,
//! matching upstream's `node.body.child_nodes.map(&:source).join("\n")`.
//!
//! # `case`/`case-in` without a match var
//!
//! `on_case_match`'s `else` branch (`case_match_node.in_pattern_branches`
//! when there's no top `condition`) is dead code even upstream: Ruby's
//! grammar requires a predicate expression for every `case...in`, unlike
//! `case...when`, so `case_match_node.condition` is never `nil` in valid
//! source. It is omitted here for the same reason.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{
    AndNode, ArrayNode, BeginNode, CallNode, CaseMatchNode, CaseNode, ElseNode, IfNode, OrNode,
    StatementsNode, UnlessNode, UntilNode, WhileNode,
};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks of literals used in conditions.
#[derive(Debug, Clone)]
pub struct LiteralAsCondition;

impl Rule for LiteralAsCondition {
    const META: RuleMeta = RuleMeta {
        name: "Lint/LiteralAsCondition",
        department: Department::Lint,
        summary: "Checks of literals used in conditions.",
        explanation: "\
Checks for literals used as the conditions or as
operands in and/or expressions serving as the conditions of
if/while/until/case-when/case-in.

NOTE: Literals in `case-in` condition where the match variable is used in
`in` are accepted as a pattern matching.

```ruby
# bad
if 20
  do_something
end

# bad
# We're only interested in the left hand side being a truthy literal,
# because it affects the evaluation of the &&, whereas the right hand
# side will be conditionally executed/called and can be a literal.
if true && some_var
  do_something
end

# good
if some_var
  do_something
end

# good
# When using a boolean value for an infinite loop.
while true
  break if condition
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::AndNode,
            NodeKind::OrNode,
            NodeKind::IfNode,
            NodeKind::UnlessNode,
            NodeKind::WhileNode,
            NodeKind::UntilNode,
            NodeKind::CaseNode,
            NodeKind::CaseMatchNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => on_send(&node.as_call_node().expect("kind matched"), ctx),
            NodeKind::AndNode => on_and(&node.as_and_node().expect("kind matched"), ctx),
            NodeKind::OrNode => on_or(&node.as_or_node().expect("kind matched"), ctx),
            NodeKind::IfNode => on_if(&node.as_if_node().expect("kind matched"), ctx),
            NodeKind::UnlessNode => on_unless(&node.as_unless_node().expect("kind matched"), ctx),
            NodeKind::WhileNode => on_while(&node.as_while_node().expect("kind matched"), ctx),
            NodeKind::UntilNode => on_until(&node.as_until_node().expect("kind matched"), ctx),
            NodeKind::CaseNode => on_case(&node.as_case_node().expect("kind matched"), ctx),
            NodeKind::CaseMatchNode => {
                on_case_match(&node.as_case_match_node().expect("kind matched"), ctx);
            }
            _ => {}
        }
    }
}

/// RuboCop-AST's `TRUTHY_LITERALS`.
fn is_truthy_literal(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::TrueNode
            | NodeKind::RangeNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
    )
}

/// RuboCop-AST's `FALSEY_LITERALS`.
fn is_falsey_literal(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::FalseNode | NodeKind::NilNode)
}

/// RuboCop-AST's `Node#literal?`.
fn is_literal(node: &Node<'_>) -> bool {
    is_truthy_literal(node) || is_falsey_literal(node)
}

/// RuboCop-AST's `BASIC_LITERALS` (`LITERALS - COMPOSITE_LITERALS`).
fn is_basic_literal(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::StringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
    )
}

/// The cop's own `basic_literal?`: a plain basic literal, or (recursively)
/// an array of nothing but basic literals/nested such arrays.
fn is_basic_literal_or_primitive_array(node: &Node<'_>) -> bool {
    match node.as_array_node() {
        Some(array) => is_primitive_array(&array),
        None => is_basic_literal(node),
    }
}

/// The cop's own `primitive_array?`.
fn is_primitive_array(array: &ArrayNode<'_>) -> bool {
    array.elements().iter().all(|el| is_basic_literal_or_primitive_array(&el))
}

/// RuboCop's `message`: the literal's own source text, formatted into
/// `MSG`.
fn literal_message(ctx: &Context<'_>, span: Span) -> String {
    format!("Literal `{}` appeared as a condition.", String::from_utf8_lossy(ctx.text(span)))
}

fn report_literal(ctx: &mut Context<'_>, span: Span) {
    let message = literal_message(ctx, span);
    ctx.report(&LiteralAsCondition::META, span, message);
}

/// RuboCop's `on_and`.
fn on_and(node: &AndNode<'_>, ctx: &mut Context<'_>) {
    let lhs = node.left();
    if !is_truthy_literal(&lhs) {
        return;
    }
    let message = literal_message(ctx, lhs.span());
    let rhs = node.right();
    if is_void_value_expression(rhs) {
        ctx.report(&LiteralAsCondition::META, lhs.span(), message);
        return;
    }
    let replacement = ctx.text(rhs.span()).to_vec();
    ctx.report_with_fix(
        &LiteralAsCondition::META,
        lhs.span(),
        message,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node.as_node().span(), replacement)],
        },
    );
}

/// RuboCop's `on_or`.
fn on_or(node: &OrNode<'_>, ctx: &mut Context<'_>) {
    let lhs = node.left();
    if !is_falsey_literal(&lhs) {
        return;
    }
    let message = literal_message(ctx, lhs.span());
    let rhs = node.right();
    if is_void_value_expression(rhs) {
        ctx.report(&LiteralAsCondition::META, lhs.span(), message);
        return;
    }
    let replacement = ctx.text(rhs.span()).to_vec();
    ctx.report_with_fix(
        &LiteralAsCondition::META,
        lhs.span(),
        message,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node.as_node().span(), replacement)],
        },
    );
}

/// RuboCop's `void_value_expression?`: drills through nested parens
/// (whitequark's `begin_type?` loop, taking the last statement each time)
/// down to the terminal node, then checks whether that is a bare
/// `return`/`break`/`next`.
fn is_void_value_expression(mut node: Node<'_>) -> bool {
    while let Some(body) = node.as_parentheses_node().and_then(|p| p.body()) {
        match body.as_statements_node().and_then(|s| s.body().last()) {
            Some(last) => node = last,
            None => return false,
        }
    }
    matches!(node.kind(), NodeKind::ReturnNode | NodeKind::BreakNode | NodeKind::NextNode)
}

/// RuboCop's `on_send` (`RESTRICT_ON_SEND = [:!]`) plus `check_for_literal`.
fn on_send(node: &CallNode<'_>, ctx: &mut Context<'_>) {
    if node.name().as_slice() != b"!" {
        return;
    }
    let Some(receiver) = node.receiver() else { return };
    if is_literal(&receiver) {
        report_literal(ctx, receiver.span());
    } else {
        check_node(&receiver, ctx);
    }
}

/// RuboCop's `check_node`.
fn check_node(node: &Node<'_>, ctx: &mut Context<'_>) {
    if let Some(call) = node.as_call_node() {
        if call.name().as_slice() == b"!"
            && call.message_loc().is_some_and(|l| l.as_slice() == b"!")
        {
            if let Some(receiver) = call.receiver() {
                handle_node(&receiver, false, ctx);
            }
        }
    } else if let Some(and) = node.as_and_node() {
        handle_node(&and.left(), true, ctx);
        handle_node(&and.right(), true, ctx);
    } else if let Some(or) = node.as_or_node() {
        handle_node(&or.left(), false, ctx);
        handle_node(&or.right(), false, ctx);
    } else if let Some(paren) = node.as_parentheses_node() {
        if let Some(sole) = paren.body().and_then(|b| b.as_statements_node()).and_then(|s| {
            let body = s.body();
            (body.len() == 1).then(|| body.first().expect("checked len == 1"))
        }) {
            handle_node(&sole, false, ctx);
        }
    }
}

/// RuboCop's `handle_node`. `parent_is_and` is upstream's
/// `node.parent.and_type?`: a literal directly under an `AndNode` is
/// skipped here since `on_and` already reports it independently for that
/// same `AndNode`.
fn handle_node(node: &Node<'_>, parent_is_and: bool, ctx: &mut Context<'_>) {
    if is_literal(node) {
        if parent_is_and {
            return;
        }
        report_literal(ctx, node.span());
    } else if matches!(
        node.kind(),
        NodeKind::CallNode | NodeKind::AndNode | NodeKind::OrNode | NodeKind::ParenthesesNode
    ) {
        check_node(node, ctx);
    }
}

/// RuboCop's `on_if` plus `correct_if_node`/`condition_evaluation?`,
/// covering `if`, `elsif` (reached through [`IfNode::subsequent`]), and
/// ternaries (`if_keyword_loc: None`).
fn on_if(node: &IfNode<'_>, ctx: &mut Context<'_>) {
    let cond = node.predicate();
    if !(is_truthy_literal(&cond) || is_falsey_literal(&cond)) {
        return;
    }
    let is_elsif = node.if_keyword_loc().is_some_and(|k| k.as_slice() == b"elsif");
    let result = is_truthy_literal(&cond);

    let surviving_present = if result {
        node.statements().is_some()
    } else {
        match classify_subsequent(node.subsequent()) {
            Some(Subsequent::Elsif(_)) => true,
            Some(Subsequent::Else(e)) => e.statements().is_some(),
            None => false,
        }
    };
    if !surviving_present && (is_elsif || node.subsequent().is_some()) {
        return;
    }

    let new_text: Vec<u8> = if is_elsif && result {
        let branch = node.statements().expect("guarded: surviving branch present");
        let span = extend_through_trailing_comment(ctx, branch.as_node().span());
        let mut out = b"else\n  ".to_vec();
        out.extend_from_slice(ctx.text(span));
        out
    } else if is_elsif && !result {
        let mut out = b"else\n  ".to_vec();
        out.extend_from_slice(&else_branch_text(node, ctx));
        out
    } else if node.statements().is_some() && result {
        ctx.text(node.statements().expect("checked Some").as_node().span()).to_vec()
    } else if matches!(classify_subsequent(node.subsequent()), Some(Subsequent::Elsif(_))) {
        let Some(Subsequent::Elsif(elsif)) = classify_subsequent(node.subsequent()) else {
            unreachable!("just matched")
        };
        let text = ctx.text(if_own_span(&elsif)).to_vec();
        let mut out = replace_first(&text, b"elsif", b"if");
        out.extend_from_slice(b"\nend");
        out
    } else if node.subsequent().is_some() {
        else_branch_text(node, ctx)
    } else {
        Vec::new()
    };

    let message = literal_message(ctx, cond.span());
    let span = if_own_span(node);
    ctx.report_with_fix(
        &LiteralAsCondition::META,
        cond.span(),
        message,
        Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(span, new_text)] },
    );
}

/// What immediately follows an `IfNode`: another `elsif` link, or a real
/// `else` clause.
enum Subsequent<'pr> {
    Elsif(IfNode<'pr>),
    Else(ElseNode<'pr>),
}

fn classify_subsequent(node: Option<Node<'_>>) -> Option<Subsequent<'_>> {
    let node = node?;
    if let Some(elsif) = node.as_if_node() {
        Some(Subsequent::Elsif(elsif))
    } else {
        node.as_else_node().map(Subsequent::Else)
    }
}

/// RuboCop's `node.else_branch.source`: the source text of whatever
/// immediately follows this link -- a nested `elsif` link's own
/// whitequark-shaped range (see the module doc), or a plain trailing
/// `else` clause's body statements (no `else` keyword).
fn else_branch_text(node: &IfNode<'_>, ctx: &Context<'_>) -> Vec<u8> {
    match classify_subsequent(node.subsequent()) {
        Some(Subsequent::Elsif(elsif)) => ctx.text(if_own_span(&elsif)).to_vec(),
        Some(Subsequent::Else(else_node)) => else_node
            .statements()
            .map(|s| ctx.text(s.as_node().span()).to_vec())
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// See the module doc: reconstructs whitequark's per-link `source_range`
/// for an `if`/`elsif` node -- a head (`if_keyword_loc` reading `"if"`)
/// always runs through the real, shared terminal `end`; an `elsif` link
/// stops at the end of its own last content, excluding that `end`; a
/// ternary (`if_keyword_loc: None`) already has the right span from Prism
/// directly.
fn if_own_span(node: &IfNode<'_>) -> Span {
    let Some(kw) = node.if_keyword_loc() else { return node.as_node().span() };
    if kw.as_slice() != b"elsif" {
        // A modifier `if`/`unless` has no `end`, and its body sits
        // *before* the keyword (`top if cond`), so the node's own start
        // is not the keyword's start.
        let start = node.as_node().span().start;
        let end =
            node.end_keyword_loc().map_or_else(|| node.as_node().span().end, |e| e.span().end);
        return Span::new(start, end);
    }
    let end = match classify_subsequent(node.subsequent()) {
        None => node
            .statements()
            .map_or_else(|| node.predicate().span().end, |s| s.as_node().span().end),
        Some(Subsequent::Elsif(next)) => if_own_span(&next).end,
        Some(Subsequent::Else(else_node)) => else_node
            .statements()
            .map_or_else(|| else_node.else_keyword_loc().span().end, |s| s.as_node().span().end),
    };
    Span::new(kw.span().start, end)
}

/// RuboCop's `range_with_comments`, narrowed to a single trailing same-line
/// comment (the only shape `if_own_span`'s callers ever need folded back
/// in).
fn extend_through_trailing_comment(ctx: &Context<'_>, span: Span) -> Span {
    let last_line = ctx.line_col(span.end.saturating_sub(1)).line;
    match ctx.comments().iter().find(|c| c.line == last_line && c.span.start >= span.end) {
        Some(c) => Span::new(span.start, c.span.end),
        None => span,
    }
}

/// Replaces the first occurrence of `from` with `to` (Ruby's
/// `String#sub`).
fn replace_first(haystack: &[u8], from: &'static [u8], to: &'static [u8]) -> Vec<u8> {
    match haystack.windows(from.len()).position(|w| w == from) {
        Some(pos) => {
            let mut out = Vec::with_capacity(haystack.len() - from.len() + to.len());
            out.extend_from_slice(&haystack[..pos]);
            out.extend_from_slice(to);
            out.extend_from_slice(&haystack[pos + from.len()..]);
            out
        }
        None => haystack.to_vec(),
    }
}

/// RuboCop's `on_if` for `unless` nodes (`condition_evaluation?`'s
/// `node.unless?` branch): no `elsif` chain, so no recursive
/// [`if_own_span`]-style range reconstruction is needed -- `unless` always
/// owns its own real, single `end`.
fn on_unless(node: &UnlessNode<'_>, ctx: &mut Context<'_>) {
    let cond = node.predicate();
    if !(is_truthy_literal(&cond) || is_falsey_literal(&cond)) {
        return;
    }
    let result = is_falsey_literal(&cond);
    let has_else = node.else_clause().is_some();
    let surviving_present = if result {
        node.statements().is_some()
    } else {
        node.else_clause().is_some_and(|e| e.statements().is_some())
    };
    if !surviving_present && has_else {
        return;
    }

    let new_text: Vec<u8> = if node.statements().is_some() && result {
        ctx.text(node.statements().expect("checked Some").as_node().span()).to_vec()
    } else if has_else {
        node.else_clause()
            .and_then(|e| e.statements())
            .map(|s| ctx.text(s.as_node().span()).to_vec())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let message = literal_message(ctx, cond.span());
    let start = node.as_node().span().start;
    let end = node.end_keyword_loc().map_or_else(|| node.as_node().span().end, |e| e.span().end);
    let span = Span::new(start, end);
    ctx.report_with_fix(
        &LiteralAsCondition::META,
        cond.span(),
        message,
        Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(span, new_text)] },
    );
}

/// RuboCop's `on_while`/`on_while_post`.
fn on_while(node: &WhileNode<'_>, ctx: &mut Context<'_>) {
    let cond = node.predicate();
    if cond.kind() == NodeKind::TrueNode {
        return;
    }
    let message = literal_message(ctx, cond.span());
    if is_truthy_literal(&cond) {
        ctx.report_with_fix(
            &LiteralAsCondition::META,
            cond.span(),
            message,
            replace_fix(cond.span(), b"true".to_vec()),
        );
    } else if is_falsey_literal(&cond) {
        let replacement =
            if node.is_begin_modifier() { unroll_body(node.statements(), ctx) } else { Vec::new() };
        ctx.report_with_fix(
            &LiteralAsCondition::META,
            cond.span(),
            message,
            replace_fix(node.as_node().span(), replacement),
        );
    }
}

/// RuboCop's `on_until`/`on_until_post`.
fn on_until(node: &UntilNode<'_>, ctx: &mut Context<'_>) {
    let cond = node.predicate();
    if cond.kind() == NodeKind::FalseNode {
        return;
    }
    let message = literal_message(ctx, cond.span());
    if is_falsey_literal(&cond) {
        ctx.report_with_fix(
            &LiteralAsCondition::META,
            cond.span(),
            message,
            replace_fix(cond.span(), b"false".to_vec()),
        );
    } else if is_truthy_literal(&cond) {
        let replacement =
            if node.is_begin_modifier() { unroll_body(node.statements(), ctx) } else { Vec::new() };
        ctx.report_with_fix(
            &LiteralAsCondition::META,
            cond.span(),
            message,
            replace_fix(node.as_node().span(), replacement),
        );
    }
}

fn replace_fix(span: Span, replacement: Vec<u8>) -> Fix {
    Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(span, replacement)] }
}

/// RuboCop's `node.body.child_nodes.map(&:source).join("\n")`: Prism wraps
/// a post-condition loop's `begin...end` body in a `StatementsNode`
/// holding a single [`BeginNode`] (see the module doc); this unwraps that
/// one level to reach the real statements before joining their own source
/// texts.
fn unroll_body(statements: Option<StatementsNode<'_>>, ctx: &Context<'_>) -> Vec<u8> {
    let Some(stmts) = statements else { return Vec::new() };
    let body = stmts.body();
    let items: Vec<Node<'_>> = match body.first().and_then(|n| n.as_begin_node()) {
        Some(begin) => begin_statements(&begin),
        None => body.iter().collect(),
    };
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(b'\n');
        }
        out.extend_from_slice(ctx.text(item.span()));
    }
    out
}

fn begin_statements<'pr>(begin: &BeginNode<'pr>) -> Vec<Node<'pr>> {
    begin.statements().map(|s| s.body().iter().collect()).unwrap_or_default()
}

/// RuboCop's `on_case`.
fn on_case(node: &CaseNode<'_>, ctx: &mut Context<'_>) {
    match node.predicate() {
        Some(cond) => {
            if !(is_truthy_literal(&cond) || is_falsey_literal(&cond)) {
                return;
            }
            check_case(&cond, ctx);
        }
        None => {
            for when in &node.conditions() {
                let Some(when_node) = when.as_when_node() else { continue };
                let conditions = when_node.conditions();
                if conditions.is_empty() || !conditions.iter().all(|c| is_literal(&c)) {
                    continue;
                }
                let first = conditions.first().expect("checked non-empty");
                let last = conditions.last().expect("checked non-empty");
                let span = Span::new(first.span().start, last.span().end);
                report_literal(ctx, span);
            }
        }
    }
}

/// RuboCop's `on_case_match`. The predicate-less branch is dead code even
/// upstream (see the module doc) and is not ported.
fn on_case_match(node: &CaseMatchNode<'_>, ctx: &mut Context<'_>) {
    let Some(cond) = node.predicate() else { return };
    if has_match_var(&node.as_node()) {
        return;
    }
    check_case(&cond, ctx);
}

/// RuboCop's `check_case`.
fn check_case(cond: &Node<'_>, ctx: &mut Context<'_>) {
    if let Some(array) = cond.as_array_node() {
        if !is_primitive_array(&array) {
            return;
        }
    }
    if cond.kind() == NodeKind::InterpolatedStringNode {
        return;
    }
    handle_node(cond, false, ctx);
}

/// RuboCop's `case_match_node.descendants.any?(&:match_var_type?)`: a bare
/// pattern-matching capture (`in x`) or a `=>` capture's target (`in
/// Integer => m`) is a [`NodeKind::LocalVariableTargetNode`] in Prism.
fn has_match_var(node: &Node<'_>) -> bool {
    let mut found = false;
    each_descendant(node, &mut |n| {
        if n.kind() == NodeKind::LocalVariableTargetNode {
            found = true;
        }
    });
    found
}
