//! `Style/NegatedIfElseCondition`, ported from RuboCop's
//! `lib/rubocop/cop/style/negated_if_else_condition.rb`.
//!
//! whitequark unifies `elsif` and a nested `if` written after an explicit
//! `else` keyword into the same shape (the nested `if` node sits directly as
//! `else_branch`), distinguished only by `IfNode#elsif?` reading the node's
//! own `loc.keyword` text. Prism keeps these structurally distinct already:
//! an `elsif` clause is the `IfNode`'s own `subsequent()` (another
//! `IfNode`), while a nested `if` written after a literal `else` keyword
//! sits inside that `else`'s `ElseNode::statements()`. So `if_else?` here
//! only needs to check that `subsequent()` is `Some(ElseNode)` (not another
//! `IfNode`, i.e. not an `elsif` continuation) with non-empty statements,
//! and a node is itself skipped as an elsif via its own `if_keyword_loc()`
//! text (`"elsif"` vs `"if"`, absent entirely for a ternary).
//!
//! whitequark's `begin`/`kwbegin` grouping (unwrapped by
//! `unwrap_begin_nodes` down to the first child, matching the parser gem's
//! `children.first` regardless of how many statements are actually inside)
//! is Prism's [`NodeKind::ParenthesesNode`] (explicit `(...)`) and
//! [`NodeKind::BeginNode`] (an explicit `begin...end` keyword block with no
//! `rescue`/`ensure` clause of its own); both always wrap their content in a
//! `StatementsNode`, whose first statement is taken the same way.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Invert the negated condition and swap the %<type>s branches.";

/// Checks for uses of `if-else` and ternary operators with a negated
/// condition which can be simplified by inverting condition and swapping
/// branches.
#[derive(Debug, Clone, Default)]
pub struct NegatedIfElseCondition {
    /// `@corrected_nodes`: spans of `IfNode`s actually corrected so far in
    /// this investigation, reset per file since the rule is cloned fresh
    /// for each parse. `corrected_ancestor?` walks the live ancestor stack
    /// looking for a span already in this set.
    corrected: Vec<Span>,
}

impl Rule for NegatedIfElseCondition {
    const META: RuleMeta = RuleMeta {
        name: "Style/NegatedIfElseCondition",
        department: Department::Style,
        summary: "Checks for uses of `if-else` and ternary operators with a negated condition \
            which can be simplified by inverting condition and swapping branches.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let if_node = node.as_if_node().expect("kind matched");

        // `!node.elsif?`: this node is itself reached as an ancestor's
        // `elsif` continuation (its own keyword reads "elsif").
        if if_node.if_keyword_loc().is_some_and(|loc| ctx.text(loc.span()) == b"elsif") {
            return;
        }

        // `if_else?`: a genuine (non-"elsif") `else` clause with a body.
        let Some(subsequent) = if_node.subsequent() else { return };
        let Some(else_node) = subsequent.as_else_node() else { return };
        if else_node.statements().is_none() {
            return;
        }

        let Some(condition) = unwrap_begin_nodes(if_node.predicate()) else { return };
        if double_negation(&condition) || !negated_condition(&condition) {
            return;
        }
        let call = condition.as_call_node().expect("negated_condition checked send_type");
        if call.arguments().is_some_and(|a| a.arguments().len() >= 2) {
            return;
        }

        let is_ternary = if_node.if_keyword_loc().is_none();
        let kind = if is_ternary { "ternary" } else { "if-else" };
        let message = MSG.replacen("%<type>s", kind, 1);

        let span = node.span();
        let corrected_ancestor = ctx
            .ancestors()
            .iter()
            .any(|info| info.kind == NodeKind::IfNode && self.corrected.contains(&info.span));

        if corrected_ancestor {
            ctx.report(&Self::META, span, message);
            return;
        }

        let condition_replacement = correct_negated_condition(ctx, &call);
        let mut edits = vec![Edit::replace(condition.span(), condition_replacement.into_bytes())];

        if if_node.statements().is_none() {
            // `node.if_branch.nil?`: the if-branch is empty; drop the
            // `else` line wholesale so its body merges into the if-branch.
            let else_line = ctx.whole_lines(else_node.else_keyword_loc().span());
            edits.push(Edit::delete(else_line));
        } else {
            let if_range = if_range(&if_node);
            let else_range = else_range(&if_node, &else_node);
            let if_text = ctx.text(if_range).to_vec();
            let else_text = ctx.text(else_range).to_vec();
            edits.push(Edit::replace(if_range, else_text));
            edits.push(Edit::replace(else_range, if_text));
        }

        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
        self.corrected.push(span);
    }
}

/// RuboCop's `unwrap_begin_nodes`: unwraps an explicit `(...)` grouping or a
/// bare `begin...end` keyword block (with no `rescue`/`ensure` of its own)
/// down to its first statement, mirroring the parser gem's
/// `node.children.first` (taken unconditionally, not just for a single
/// statement).
fn unwrap_begin_nodes(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        if let Some(parens) = node.as_parentheses_node() {
            let stmts = parens.body()?.as_statements_node()?;
            node = stmts.body().iter().next()?;
        } else if let Some(begin) = node.as_begin_node() {
            if begin.rescue_clause().is_some() || begin.ensure_clause().is_some() {
                return Some(node);
            }
            let stmts = begin.statements()?;
            node = stmts.body().iter().next()?;
        } else {
            return Some(node);
        }
    }
}

/// RuboCop's `double_negation?`: `(send (send _ :!) :!)`.
fn double_negation(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.name().as_slice() != b"!" {
        return false;
    }
    call.receiver()
        .is_some_and(|r| r.as_call_node().is_some_and(|inner| inner.name().as_slice() == b"!"))
}

/// RuboCop's `negated_condition?`: `node.send_type? && (node.negation_method?
/// || NEGATED_EQUALITY_METHODS.include?(node.method_name))`.
fn negated_condition(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    !call.is_safe_navigation() && matches!(call.name().as_slice(), b"!" | b"!=" | b"!~")
}

/// RuboCop's `correct_negated_condition`.
fn correct_negated_condition(ctx: &Context<'_>, call: &CallNode<'_>) -> String {
    let receiver = call.receiver().expect("negated call always has a receiver");
    let receiver_src = String::from_utf8_lossy(ctx.text(receiver.span()));
    if call.name().as_slice() == b"!" {
        return receiver_src.into_owned();
    }
    let name = call.name().as_slice();
    let inverted: Vec<u8> = name.iter().map(|&b| if b == b'!' { b'=' } else { b }).collect();
    let inverted = String::from_utf8_lossy(&inverted);
    let arg = call
        .arguments()
        .and_then(|a| a.arguments().iter().next())
        .expect("!=/!~ always has one argument");
    let arg_src = String::from_utf8_lossy(ctx.text(arg.span()));
    format!("{receiver_src} {inverted} {arg_src}")
}

/// RuboCop's `if_range`: the if-branch node itself for the ternary shape
/// (its single statement), or the whole text between the condition and the
/// `else` keyword (including interleaved whitespace/comments) for the
/// if/else shape.
fn if_range(if_node: &ruby_ast::node::IfNode<'_>) -> Span {
    if if_node.if_keyword_loc().is_none() {
        // Ternary: the if-branch is the condition's sole statement.
        if_node
            .statements()
            .expect("ternary always has an if-branch")
            .body()
            .iter()
            .next()
            .expect("ternary statements is non-empty")
            .span()
    } else {
        let condition_end = if_node.predicate().span().end;
        let else_begin = if_node
            .subsequent()
            .expect("if_else? checked")
            .as_else_node()
            .expect("if_else? checked")
            .else_keyword_loc()
            .span()
            .start;
        Span::new(condition_end, else_begin)
    }
}

/// RuboCop's `else_range`.
fn else_range(
    if_node: &ruby_ast::node::IfNode<'_>,
    else_node: &ruby_ast::node::ElseNode<'_>,
) -> Span {
    if if_node.if_keyword_loc().is_none() {
        else_node
            .statements()
            .expect("if_else? checked non-empty")
            .body()
            .iter()
            .next()
            .expect("non-empty")
            .span()
    } else {
        let else_end = else_node.else_keyword_loc().span().end;
        let end_begin =
            if_node.end_keyword_loc().expect("if/else always has an end keyword").span().start;
        Span::new(else_end, end_begin)
    }
}
