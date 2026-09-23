//! `Style/RedundantParentheses`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_parentheses.rb` plus the `Parentheses`
//! mixin (`lib/rubocop/cop/mixin/parentheses.rb`) it includes and the
//! `ParenthesesCorrector` it autocorrects with.
//!
//! # Prism/whitequark shape differences
//!
//! whitequark represents *every* parenthesized group -- including a bare
//! `(x)` -- as a single `begin` node whose `children` are the statements
//! directly (no extra wrapper for a lone statement); a `begin` node's
//! `parentheses?` is true only when it carries real `(`/`)` locations.
//! Prism instead always wraps a [`NodeKind::ParenthesesNode`]'s body in a
//! [`NodeKind::StatementsNode`], and separately wraps *any* multi-statement
//! or single-statement body position (method/`if`/block bodies, ...) in a
//! `StatementsNode` even when whitequark would use the child node directly
//! with no wrapper at all. So: a `StatementsNode` with exactly one child is
//! transparent (whitequark elides it); with zero or 2+ children it *is*
//! whitequark's implicit (unparenthesized) `begin`. Prism's `ArgumentsNode`
//! (call/`super`/`yield` argument list) and `ElseNode` (`else` clause
//! wrapper) don't exist as distinct nodes in whitequark at all and are
//! always transparent. [`normalized_chain`] builds the whitequark-shaped
//! ancestor chain by skipping these wrappers over the engine's raw
//! [`Context::ancestors`].
//!
//! Parent-dependent predicates (`parent.type?`, `parent.ternary?`, ...)
//! need data beyond a bare `NodeKind`, which the ancestor stack alone
//! doesn't carry (it stores kind + span only). [`Facts`] records exactly
//! that extra data -- computed once, in [`RedundantParentheses::enter`],
//! for every node kind that can be a meaningful ancestor of a parens node --
//! keyed by `(span, kind)` and looked up through the normalized chain,
//! mirroring `hash_syntax.rs`'s `AncestorView`/`Facts` cache so the whole
//! file is still walked exactly once.
//!
//! The pin operator (`^(var)`) is a second, unrelated entry point: Prism's
//! `PinnedExpressionNode` carries its own `lparen_loc`/`rparen_loc` and
//! exposes the bare inner expression directly -- there is no
//! `ParenthesesNode` at all here, so it can never reach the main
//! `ParenthesesNode` handler. RuboCop's own `allowed_pin_operator?` matcher
//! only ever produces an offense (always "a variable") when the pinned
//! expression is itself a bare variable read, so that one case is handled
//! as a small special case in [`RedundantParentheses::check_pin`].
//!
//! # Blind spots (documented in `META.blind_spots`)
//! A handful of rarely-hit predicates are approximated in the direction of
//! false negatives: `first_arg_begins_with_hash_literal?` only looks at the
//! immediate call (not a full ancestor climb), one-line-rescue and
//! multi-statement-parens edge cases, `call_chain_starts_with_int?`,
//! `do_end_block_in_method_chain?`, and the heredoc-comma corrector
//! special case are not ported. `Style/ParenthesesAroundCondition`'s
//! `AllowInMultilineConditions` is treated as always `false` (its
//! RuboCop-wide default) because peer options can't expose whether that
//! cop is itself enabled, which is required to reproduce `for_enabled_cop`.

use std::borrow::Cow;
use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::NodeList;
use ruby_ast::{LocationExt, Node, NodeExt, NodeKind};
use ruby_source::Span;

const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

const COMPARISON_OPERATORS: &[&[u8]] = &[b"==", b"===", b"!=", b"<=", b">=", b">", b"<"];

/// A node identity: span plus kind, unique enough to key the ancestor-facts
/// cache (mirrors `hash_syntax.rs`'s `Key`).
type Key = (Span, NodeKind);

/// Extra data about one node that a normalized-ancestor lookup can't read
/// off a bare `(span, kind)` pair, gathered once when the node itself is
/// visited (own facts, for descendant/ancestor lookups) or computed fresh
/// on demand (content facts, since a `ParenthesesNode`'s content hasn't
/// been visited yet when the parens themselves are entered).
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default)]
struct Facts {
    /// `CallNode`/`SuperNode`/`ForwardingSuperNode`/`YieldNode`/`DefinedNode`:
    /// the receiver's span, if any (only meaningful for `CallNode`).
    receiver_span: Option<Span>,
    /// Whether the node's own call/keyword syntax carries real `(...)`.
    has_own_parens: bool,
    /// Number of arguments (0 for `ForwardingSuperNode`/no-arg forms).
    args_count: usize,
    /// The first argument's span, if any.
    first_arg_span: Option<Span>,
    /// `CallNode` method name.
    name: Option<Box<[u8]>>,
    /// `CallNode`: whether the call uses `.`/`&.` (vs. bare/operator syntax).
    has_dot: bool,
    /// `ArrayNode`: number of elements.
    elements_count: usize,
    is_operator_method: bool,
    is_unary_operation: bool,
    is_prefix_not: bool,
    is_comparison_method: bool,
    /// `IfNode`/`UnlessNode`/`WhileNode`/`UntilNode`: the condition's span.
    condition_span: Option<Span>,
    /// `IfNode`: true for the ternary (`? :`) form.
    is_ternary: bool,
    /// `WhileNode`/`UntilNode`: true for the post-condition (`begin...end
    /// while`) form.
    is_begin_modifier: bool,
    /// `DefNode`: true for the endless (`def foo = ...`) form.
    is_endless: bool,
    /// `AndNode`/`OrNode`: true when the operator is spelled `and`/`or`
    /// rather than `&&`/`||`.
    is_semantic_operator: bool,
    /// `StatementsNode`: number of statements in the body.
    stmt_count: usize,
}

fn is_ascii_lower(b: u8) -> bool {
    b.is_ascii_lowercase()
}

/// RuboCop's `Parentheses#parens_required?`: true when removing the parens
/// would glue an identifier/keyword onto the surrounding text.
fn parens_required(ctx: &Context<'_>, span: Span) -> bool {
    let bytes = ctx.source().bytes();
    let before =
        span.start > 0 && bytes.get(span.start as usize - 1).copied().is_some_and(is_ascii_lower);
    let after = bytes.get(span.end as usize).copied().is_some_and(is_ascii_lower);
    before || after
}

fn line_of(ctx: &Context<'_>, offset: u32) -> u32 {
    ctx.source().line_col(offset).line
}

/// RuboCop-AST's `Node#multiline?` for a node's own span.
fn is_multiline(ctx: &Context<'_>, span: Span) -> bool {
    let end = if span.end > span.start { span.end - 1 } else { span.start };
    line_of(ctx, span.start) != line_of(ctx, end)
}

/// Builds [`Facts`] for the node kinds this rule cares about; `None` for
/// every other kind. Called both when recording ancestor facts (in
/// [`RedundantParentheses::enter`]) and directly on a parens' live content
/// node (which hasn't been visited by the traversal yet).
#[allow(clippy::too_many_lines)]
fn build_facts(node: &Node<'_>, ctx: &Context<'_>) -> Option<Facts> {
    let mut f = Facts::default();
    match node.kind() {
        NodeKind::CallNode => {
            let call = node.as_call_node()?;
            let name = call.name().as_slice();
            f.receiver_span = call.receiver().map(|r| r.span());
            f.has_dot = call
                .call_operator_loc()
                .is_some_and(|o| matches!(ctx.text(o.span()), b"." | b"&."));
            f.has_own_parens =
                call.opening_loc().is_some_and(|o| ctx.text(o.span()).first() == Some(&b'('));
            let args = call.arguments();
            f.args_count = args.as_ref().map_or(0, |a| a.arguments().len());
            f.first_arg_span =
                args.as_ref().and_then(|a| a.arguments().iter().next()).map(|n| n.span());
            f.is_operator_method = OPERATOR_METHODS.contains(&name);
            f.is_comparison_method = COMPARISON_OPERATORS.contains(&name);
            let message_start = call.message_loc().map(|l| l.span().start);
            f.is_unary_operation =
                f.is_operator_method && message_start.is_some_and(|m| m == node.span().start);
            f.is_prefix_not = name == b"!"
                && call.receiver().is_some()
                && call.message_loc().is_some_and(|l| ctx.text(l.span()) == b"not");
            f.name = Some(name.to_vec().into_boxed_slice());
        }
        NodeKind::SuperNode => {
            let s = node.as_super_node()?;
            f.has_own_parens = s.lparen_loc().is_some();
            let args = s.arguments();
            f.args_count = args.as_ref().map_or(0, |a| a.arguments().len());
            f.first_arg_span =
                args.as_ref().and_then(|a| a.arguments().iter().next()).map(|n| n.span());
        }
        NodeKind::ForwardingSuperNode => {
            f.has_own_parens = false;
            f.args_count = 0;
        }
        NodeKind::YieldNode => {
            let y = node.as_yield_node()?;
            f.has_own_parens = y.lparen_loc().is_some();
            let args = y.arguments();
            f.args_count = args.as_ref().map_or(0, |a| a.arguments().len());
            f.first_arg_span =
                args.as_ref().and_then(|a| a.arguments().iter().next()).map(|n| n.span());
        }
        NodeKind::DefinedNode => {
            let d = node.as_defined_node()?;
            f.has_own_parens = d.lparen_loc().is_some();
            f.args_count = 1;
        }
        NodeKind::BreakNode => {
            let n = node.as_break_node()?;
            let args = n.arguments();
            f.args_count = args.as_ref().map_or(0, |a| a.arguments().len());
            f.first_arg_span =
                args.as_ref().and_then(|a| a.arguments().iter().next()).map(|n| n.span());
        }
        NodeKind::NextNode => {
            let n = node.as_next_node()?;
            let args = n.arguments();
            f.args_count = args.as_ref().map_or(0, |a| a.arguments().len());
            f.first_arg_span =
                args.as_ref().and_then(|a| a.arguments().iter().next()).map(|n| n.span());
        }
        NodeKind::ReturnNode => {
            let n = node.as_return_node()?;
            let args = n.arguments();
            f.args_count = args.as_ref().map_or(0, |a| a.arguments().len());
            f.first_arg_span =
                args.as_ref().and_then(|a| a.arguments().iter().next()).map(|n| n.span());
        }
        NodeKind::IfNode => {
            let n = node.as_if_node()?;
            f.condition_span = Some(n.predicate().span());
            f.is_ternary = n.if_keyword_loc().is_none();
        }
        NodeKind::UnlessNode => {
            let n = node.as_unless_node()?;
            f.condition_span = Some(n.predicate().span());
        }
        NodeKind::WhileNode => {
            let n = node.as_while_node()?;
            f.condition_span = Some(n.predicate().span());
            f.is_begin_modifier = n.is_begin_modifier();
        }
        NodeKind::UntilNode => {
            let n = node.as_until_node()?;
            f.condition_span = Some(n.predicate().span());
            f.is_begin_modifier = n.is_begin_modifier();
        }
        NodeKind::CaseNode => {
            let n = node.as_case_node()?;
            f.condition_span = n.predicate().map(|p| p.span());
        }
        NodeKind::DefNode => {
            let n = node.as_def_node()?;
            f.is_endless = n.equal_loc().is_some();
        }
        NodeKind::AndNode => {
            let n = node.as_and_node()?;
            f.is_semantic_operator = ctx.text(n.operator_loc().span()) == b"and";
        }
        NodeKind::OrNode => {
            let n = node.as_or_node()?;
            f.is_semantic_operator = ctx.text(n.operator_loc().span()) == b"or";
        }
        NodeKind::StatementsNode => {
            let n = node.as_statements_node()?;
            f.stmt_count = n.body().len();
        }
        NodeKind::ArrayNode => {
            let n = node.as_array_node()?;
            f.elements_count = n.elements().len();
        }
        _ => return None,
    }
    Some(f)
}

fn variable_kind(k: NodeKind) -> bool {
    matches!(
        k,
        NodeKind::LocalVariableReadNode
            | NodeKind::InstanceVariableReadNode
            | NodeKind::ClassVariableReadNode
            | NodeKind::GlobalVariableReadNode
    )
}

fn const_kind(k: NodeKind) -> bool {
    matches!(k, NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

fn literal_kind(k: NodeKind) -> bool {
    matches!(
        k,
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
            | NodeKind::KeywordHashNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::TrueNode
            | NodeKind::RangeNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
    )
}

fn numeric_kind(k: NodeKind) -> bool {
    matches!(
        k,
        NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::RationalNode
            | NodeKind::ImaginaryNode
    )
}

/// RuboCop-AST's `ASSIGNMENTS` (`EQUALS_ASSIGNMENTS` + `SHORTHAND_ASSIGNMENTS`),
/// expanded from whitequark's generic `lvasgn`/... /`op_asgn`/`or_asgn`/
/// `and_asgn` into Prism's per-target-kind write nodes.
fn is_assignment_kind(k: NodeKind) -> bool {
    matches!(
        k,
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

/// RuboCop-AST's `Node::KEYWORDS` node-kind half (the `and`/`or`/`not`
/// semantic-operator forms are handled separately in [`is_keyword`]).
fn is_keyword_kind(k: NodeKind) -> bool {
    matches!(
        k,
        NodeKind::AliasMethodNode
            | NodeKind::AliasGlobalVariableNode
            | NodeKind::BreakNode
            | NodeKind::CaseNode
            | NodeKind::ClassNode
            | NodeKind::DefNode
            | NodeKind::DefinedNode
            | NodeKind::BeginNode
            | NodeKind::EnsureNode
            | NodeKind::ForNode
            | NodeKind::IfNode
            | NodeKind::ModuleNode
            | NodeKind::NextNode
            | NodeKind::PostExecutionNode
            | NodeKind::RedoNode
            | NodeKind::RescueNode
            | NodeKind::RetryNode
            | NodeKind::ReturnNode
            | NodeKind::SelfNode
            | NodeKind::SuperNode
            | NodeKind::ForwardingSuperNode
            | NodeKind::UndefNode
            | NodeKind::UntilNode
            | NodeKind::WhenNode
            | NodeKind::WhileNode
            | NodeKind::YieldNode
            | NodeKind::SourceFileNode
            | NodeKind::SourceLineNode
            | NodeKind::SourceEncodingNode
    )
}

fn is_keyword(k: NodeKind, facts: Option<&Facts>) -> bool {
    if is_keyword_kind(k) {
        return true;
    }
    matches!(k, NodeKind::AndNode | NodeKind::OrNode)
        && facts.is_some_and(|f| f.is_semantic_operator)
}

/// Builds the whitequark-shaped ancestor chain (nearest first) by skipping
/// Prism-only wrapper kinds over the engine's raw ancestor stack. See the
/// module doc for why each kind is (or isn't) transparent.
fn normalized_chain(ctx: &Context<'_>, facts: &HashMap<Key, Facts>) -> Vec<(Span, NodeKind)> {
    let mut out = Vec::new();
    for info in ctx.ancestors().iter().rev() {
        match info.kind {
            NodeKind::ArgumentsNode | NodeKind::ProgramNode | NodeKind::ElseNode => {}
            NodeKind::StatementsNode => {
                let single = facts.get(&(info.span, info.kind)).is_some_and(|f| f.stmt_count == 1);
                if !single {
                    out.push((info.span, info.kind));
                }
            }
            _ => out.push((info.span, info.kind)),
        }
    }
    out
}

fn chain_begins_with_hash(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::HashNode | NodeKind::KeywordHashNode => true,
        NodeKind::CallNode => node
            .as_call_node()
            .and_then(|c| c.receiver())
            .is_some_and(|r| chain_begins_with_hash(&r)),
        _ => false,
    }
}

/// RuboCop's `first_argument?`: is `span` (or is any ancestor along
/// `chain`) literally the first argument of the next call up the chain --
/// climbs the whole chain, not just the immediate parent, since the
/// parens can be nested several receiver-less levels deep (e.g. as the
/// receiver of a method chain that is itself the first argument).
fn first_argument(span: Span, chain: &[(Span, NodeKind)], facts: &HashMap<Key, Facts>) -> bool {
    let mut cur = span;
    for &(pspan, pkind) in chain {
        if matches!(pkind, NodeKind::CallNode | NodeKind::SuperNode | NodeKind::YieldNode)
            && facts.get(&(pspan, pkind)).is_some_and(|f| f.first_arg_span == Some(cur))
        {
            return true;
        }
        cur = pspan;
    }
    false
}

/// RuboCop's `node.each_ancestor(:send).to_a.last.parenthesized_call?`:
/// the outermost call ancestor's own call-parens.
fn topmost_call_parenthesized(chain: &[(Span, NodeKind)], facts: &HashMap<Key, Facts>) -> bool {
    let mut result = false;
    for &(pspan, pkind) in chain {
        if pkind == NodeKind::CallNode {
            result = facts.get(&(pspan, pkind)).is_some_and(|f| f.has_own_parens);
        }
    }
    result
}

/// The innermost receiver's kind, descending through a bare receiver
/// chain (RuboCop's `first_part_of_call_chain`).
fn innermost_receiver_kind(node: &Node<'_>) -> NodeKind {
    match node.as_call_node().and_then(|c| c.receiver()) {
        Some(recv) => innermost_receiver_kind(&recv),
        None => node.kind(),
    }
}

/// RuboCop's `call_chain_starts_with_int?`: our parens is the base of a
/// unary `-@`/`+@` whose call chain bottoms out at an integer literal
/// (e.g. `-(1.foo)`), which would re-associate if unparenthesized.
fn call_chain_starts_with_int(
    content: &Node<'_>,
    chain: &[(Span, NodeKind)],
    facts: &HashMap<Key, Facts>,
) -> bool {
    if innermost_receiver_kind(content) != NodeKind::IntegerNode {
        return false;
    }
    let Some(&(pspan, pkind)) = chain.first() else { return false };
    pkind == NodeKind::CallNode
        && facts
            .get(&(pspan, pkind))
            .is_some_and(|f| matches!(f.name.as_deref(), Some(b"-@" | b"+@")))
}

/// Whether `node`, or any node in its bare receiver chain, is a call with
/// a `do...end` (not `{}`) block attached.
fn chain_has_do_end_block(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if let Some(block) = call.block().as_ref().and_then(Node::as_block_node) {
        if ctx.text(block.opening_loc().span()).first() == Some(&b'd') {
            return true;
        }
    }
    call.receiver().is_some_and(|r| chain_has_do_end_block(&r, ctx))
}

/// RuboCop's `do_end_block_in_method_chain?`: a `do...end` block anywhere
/// in our content's own receiver chain, while we ourselves sit somewhere
/// inside a method-call ancestor (so unwrapping would change how the
/// `do...end` attaches).
fn do_end_block_in_method_chain(
    content: &Node<'_>,
    chain: &[(Span, NodeKind)],
    ctx: &Context<'_>,
) -> bool {
    chain_has_do_end_block(content, ctx) && chain.iter().any(|(_, k)| *k == NodeKind::CallNode)
}

/// Text of a JavaScript-style `\s`-adjacent run: `final_pos` from RuboCop's
/// `RangeHelp`, ported directly (its `continuations` parameter is always
/// `false` for `ParenthesesCorrector`'s two call sites, so it's dropped).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn final_pos(src: &[u8], pos: u32, step: i64, newlines: bool, whitespace: bool) -> u32 {
    let mut p = i64::from(pos);
    p = move_while(src, p, step, |b| b == b' ' || b == b'\t');
    if newlines {
        p = move_while(src, p, step, |b| b == b'\n');
    }
    if whitespace {
        p = move_while(src, p, step, |b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C));
    }
    p.max(0) as u32
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn move_while(src: &[u8], mut p: i64, step: i64, pred: impl Fn(u8) -> bool) -> i64 {
    loop {
        let look = if step < 0 { p - 1 } else { p };
        if look < 0 || look as usize >= src.len() {
            break;
        }
        if pred(src[look as usize]) {
            p += step;
        } else {
            break;
        }
    }
    p
}

/// Looks for redundant parentheses.
#[derive(Debug, Clone, Default)]
pub struct RedundantParentheses {
    facts: HashMap<Key, Facts>,
    /// `Style/TernaryParentheses`'s `EnforcedStyle` being one of
    /// `require_parentheses`/`require_parentheses_when_complex`.
    ternary_parentheses_required: bool,
    /// `Style/ParenthesesAroundCondition`'s `AllowInMultilineConditions`.
    allow_in_multiline_conditions: bool,
}

impl Rule for RedundantParentheses {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantParentheses",
        department: Department::Style,
        summary: "Checks for parentheses that seem not to serve any purpose.",
        explanation: "\
Checks for redundant parentheses that don't change the meaning of the
expression -- around a bare variable, constant, or literal, around an
assignment, logical/comparison expression, keyword, or method call/unary
operation that doesn't need them to parse.

```ruby
# bad
(x) if ((y.z).nil?)

# good
x if y.z.nil?
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::ParenthesesNode,
            NodeKind::PinnedExpressionNode,
            NodeKind::StatementsNode,
            NodeKind::CallNode,
            NodeKind::SuperNode,
            NodeKind::ForwardingSuperNode,
            NodeKind::YieldNode,
            NodeKind::DefinedNode,
            NodeKind::BreakNode,
            NodeKind::NextNode,
            NodeKind::ReturnNode,
            NodeKind::IfNode,
            NodeKind::UnlessNode,
            NodeKind::WhileNode,
            NodeKind::UntilNode,
            NodeKind::DefNode,
            NodeKind::CaseNode,
            NodeKind::ArrayNode,
            NodeKind::AndNode,
            NodeKind::OrNode,
        ],
        config: &[],
        blind_spots: "\
One-line pattern matching (`in`/`=>`) is only handled for the top-level \
`MatchPredicateNode`/`MatchRequiredNode` content case, not a full \
ancestor-walk guard for nested cases. The heredoc-trailing-comma special \
case in `ParenthesesCorrector` (`method(<<~X, ...)`) is not ported; such a \
fix is skipped rather than emitted incorrectly. `Style/TernaryParentheses`'s \
`Enabled` flag isn't checked (only its `EnforcedStyle` is read, assuming \
enabled, matching the RuboCop default) because peer options can't expose \
whether a peer cop is itself enabled; the same gap means \
`Style/ParenthesesAroundCondition`'s `AllowInMultilineConditions` is \
honored even when that cop is configured but disabled (RuboCop's \
`for_enabled_cop` would ignore it there) -- chosen because honoring the \
configured value avoids false positives in the far more common \
enabled-peer case, at the cost of false negatives in the rare \
disabled-peer-with-override case.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = options
            .peer("Style/TernaryParentheses", "EnforcedStyle")
            .and_then(|v| v.as_str())
            .map_or_else(|| "require_no_parentheses".to_string(), str::to_string);
        let allow_in_multiline_conditions = options
            .peer("Style/ParenthesesAroundCondition", "AllowInMultilineConditions")
            .and_then(linter::OptionValue::as_bool)
            .unwrap_or(false);
        Ok(Self {
            facts: HashMap::new(),
            ternary_parentheses_required: matches!(
                style.as_str(),
                "require_parentheses" | "require_parentheses_when_complex"
            ),
            allow_in_multiline_conditions,
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.facts.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ParenthesesNode => self.check_parens(node, ctx),
            NodeKind::PinnedExpressionNode => self.check_pin(node, ctx),
            kind => {
                if let Some(f) = build_facts(node, ctx) {
                    self.facts.insert((node.span(), kind), f);
                }
            }
        }
    }
}

impl RedundantParentheses {
    fn check_pin(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(pin) = node.as_pinned_expression_node() else { return };
        let lparen = pin.lparen_loc();
        let rparen = pin.rparen_loc();
        let inner = pin.expression();
        if !variable_kind(inner.kind()) {
            return;
        }
        let span = Span::new(lparen.span().start, rparen.span().end);
        self.report(ctx, span, "a variable", lparen.span(), rparen.span());
    }

    fn check_parens(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(parens) = node.as_parentheses_node() else { return };
        let span = node.span();
        let Some(body) = parens.body() else { return }; // `()`
        let Some(stmts) = body.as_statements_node() else { return };
        let list = stmts.body();
        if list.is_empty() {
            return;
        }
        let content = list.iter().next().expect("non-empty");

        let chain = normalized_chain(ctx, &self.facts);

        if self.parens_allowed(&content, span, &list, &chain, ctx) {
            return;
        }
        if self.ignore_syntax(span, &chain, ctx) {
            return;
        }

        self.check(span, content, &chain, ctx);
    }

    fn parens_allowed(
        &self,
        content: &Node<'_>,
        span: Span,
        list: &NodeList<'_>,
        chain: &[(Span, NodeKind)],
        ctx: &Context<'_>,
    ) -> bool {
        self.first_arg_begins_with_hash_literal(content, span, chain)
            || Self::rescue_ancestor(chain)
            || Self::in_pattern_matching_in_method_argument(content.kind(), chain)
            || self.allowed_expression(content, span, list, chain, ctx)
    }

    fn first_arg_begins_with_hash_literal(
        &self,
        content: &Node<'_>,
        span: Span,
        chain: &[(Span, NodeKind)],
    ) -> bool {
        if !chain_begins_with_hash(content) {
            return false;
        }
        if !first_argument(span, chain, &self.facts) {
            return false;
        }
        !topmost_call_parenthesized(chain, &self.facts)
    }

    fn rescue_ancestor(chain: &[(Span, NodeKind)]) -> bool {
        chain.first().is_some_and(|(_, k)| *k == NodeKind::RescueNode)
            || chain.get(1).is_some_and(|(_, k)| *k == NodeKind::RescueNode)
    }

    fn in_pattern_matching_in_method_argument(
        content_kind: NodeKind,
        chain: &[(Span, NodeKind)],
    ) -> bool {
        content_kind == NodeKind::MatchPredicateNode
            && chain.first().is_some_and(|(_, k)| *k == NodeKind::CallNode)
    }

    fn allowed_expression(
        &self,
        _content: &Node<'_>,
        span: Span,
        list: &NodeList<'_>,
        chain: &[(Span, NodeKind)],
        ctx: &Context<'_>,
    ) -> bool {
        self.allowed_ancestor(span, chain, ctx)
            || Self::allowed_multiple_expression(list.len(), chain)
            || self.allowed_ternary(span, chain)
            || chain.first().is_some_and(|(_, k)| *k == NodeKind::RangeNode)
    }

    fn allowed_ancestor(&self, span: Span, chain: &[(Span, NodeKind)], ctx: &Context<'_>) -> bool {
        let Some(&(pspan, pkind)) = chain.first() else { return false };
        is_keyword(pkind, self.facts.get(&(pspan, pkind))) && parens_required(ctx, span)
    }

    fn allowed_multiple_expression(stmt_count: usize, chain: &[(Span, NodeKind)]) -> bool {
        if stmt_count <= 1 {
            return false;
        }
        let Some(&(_, k)) = chain.first() else { return false };
        !matches!(
            k,
            NodeKind::ParenthesesNode
                | NodeKind::StatementsNode
                | NodeKind::DefNode
                | NodeKind::BlockNode
        )
    }

    fn allowed_ternary(&self, span: Span, chain: &[(Span, NodeKind)]) -> bool {
        let Some(&(pspan, pkind)) = chain.first() else { return false };
        if pkind != NodeKind::IfNode {
            return false;
        }
        let Some(f) = self.facts.get(&(pspan, pkind)) else { return false };
        f.is_ternary && f.condition_span == Some(span) && self.ternary_parentheses_required
    }

    fn ignore_syntax(&self, span: Span, chain: &[(Span, NodeKind)], ctx: &Context<'_>) -> bool {
        let Some(&(pspan, pkind)) = chain.first() else { return false };
        let post_loop = matches!(pkind, NodeKind::WhileNode | NodeKind::UntilNode)
            && self.facts.get(&(pspan, pkind)).is_some_and(|f| f.is_begin_modifier);
        post_loop
            || self.like_method_argument_parentheses(span, pspan, pkind)
            || multiline_control_flow_statements(pspan, pkind, ctx)
    }

    fn like_method_argument_parentheses(&self, span: Span, pspan: Span, pkind: NodeKind) -> bool {
        if !matches!(pkind, NodeKind::CallNode | NodeKind::SuperNode | NodeKind::YieldNode) {
            return false;
        }
        let Some(f) = self.facts.get(&(pspan, pkind)) else { return false };
        f.args_count == 1
            && !f.has_own_parens
            && !f.is_operator_method
            && f.first_arg_span == Some(span)
    }

    fn check(
        &mut self,
        span: Span,
        content: Node<'_>,
        chain: &[(Span, NodeKind)],
        ctx: &mut Context<'_>,
    ) {
        if let Some(msg) = self.find_offense_message(span, &content, chain, ctx) {
            let mut report_span = span;
            if content.kind() == NodeKind::RangeNode
                && !self.argument_of_parenthesized_method_call(
                    span,
                    content.kind(),
                    chain,
                    build_facts(&content, ctx).as_ref(),
                )
            {
                if let Some(&(pspan, NodeKind::ParenthesesNode)) = chain.first() {
                    report_span = pspan;
                }
            }
            self.offense(ctx, report_span, msg);
            return;
        }
        if call_node(&content) {
            self.check_send(span, content, chain, ctx);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn find_offense_message(
        &self,
        span: Span,
        content: &Node<'_>,
        chain: &[(Span, NodeKind)],
        ctx: &Context<'_>,
    ) -> Option<&'static str> {
        let content_facts = build_facts(content, ctx);
        let kind = content.kind();

        if keyword_with_redundant_parentheses(content, content_facts.as_ref(), ctx) {
            return Some("a keyword");
        }
        if literal_kind(kind) && disallowed_literal(content, span, chain, &self.facts, ctx) {
            return Some("a literal");
        }
        if variable_kind(kind) {
            return Some("a variable");
        }
        if const_kind(kind) {
            return Some("a constant");
        }
        if is_assignment_kind(kind)
            && chain.first().is_none_or(|(_, k)| {
                matches!(k, NodeKind::ParenthesesNode | NodeKind::StatementsNode)
            })
        {
            return Some("an assignment");
        }
        if is_lambda_or_proc_expression(content, ctx) {
            return Some("an expression");
        }
        if disallowed_one_line_pattern_matching(kind, chain, &self.facts) {
            return Some("a one-line pattern matching");
        }
        if interpolation(chain) {
            return Some("an interpolated expression");
        }
        if self.argument_of_parenthesized_method_call(span, kind, chain, content_facts.as_ref()) {
            return Some("a method argument");
        }
        if self.oneline_rescue_parentheses_required(span, kind, chain) {
            return Some("a one-line rescue");
        }

        if self.chained(span, chain) {
            return None;
        }

        if matches!(kind, NodeKind::AndNode | NodeKind::OrNode) {
            let f = content_facts.as_ref()?;
            if f.is_semantic_operator && !chain.is_empty() {
                return None;
            }
            if is_multiline(ctx, span) && self.allow_in_multiline_conditions {
                return None;
            }
            if let Some(&(_, pkind)) = chain.first() {
                if matches!(
                    pkind,
                    NodeKind::OrNode
                        | NodeKind::CallNode
                        | NodeKind::SplatNode
                        | NodeKind::AssocSplatNode
                ) {
                    return None;
                }
                if kind != NodeKind::AndNode && pkind == NodeKind::AndNode {
                    return None;
                }
                if pkind == NodeKind::IfNode
                    && self.facts.get(&chain[0]).is_some_and(|pf| pf.is_ternary)
                {
                    return None;
                }
            }
            return Some("a logical expression");
        } else if content_facts.as_ref().is_some_and(|f| f.is_comparison_method)
            && matches!(kind, NodeKind::CallNode)
        {
            if !chain.is_empty() {
                return None;
            }
            return Some("a comparison expression");
        }
        None
    }

    fn argument_of_parenthesized_method_call(
        &self,
        span: Span,
        content_kind: NodeKind,
        chain: &[(Span, NodeKind)],
        content_facts: Option<&Facts>,
    ) -> bool {
        if is_basic_conditional(content_kind) || content_kind == NodeKind::RescueModifierNode {
            return false;
        }
        if content_kind == NodeKind::CallNode
            && content_facts
                .is_some_and(|f| (f.receiver_span.is_none() || f.has_dot) && f.args_count > 0)
        {
            // `method_call_parentheses_required?`.
            return false;
        }
        let Some(&(pspan, pkind)) = chain.first() else { return false };
        if pkind != NodeKind::CallNode {
            return false;
        }
        let Some(f) = self.facts.get(&(pspan, pkind)) else { return false };
        f.has_own_parens && f.receiver_span != Some(span)
    }

    fn oneline_rescue_parentheses_required(
        &self,
        span: Span,
        content_kind: NodeKind,
        chain: &[(Span, NodeKind)],
    ) -> bool {
        if content_kind != NodeKind::RescueModifierNode {
            return false;
        }
        let Some(&(pspan, pkind)) = chain.first() else { return true };
        if let Some(f) = self.facts.get(&(pspan, pkind)) {
            if pkind == NodeKind::IfNode && f.is_ternary {
                return false;
            }
            if f.condition_span == Some(span) {
                return false;
            }
        }
        !matches!(pkind, NodeKind::CallNode | NodeKind::ArrayNode | NodeKind::AssocNode)
    }

    fn chained(&self, span: Span, chain: &[(Span, NodeKind)]) -> bool {
        let Some(&(pspan, pkind)) = chain.first() else { return false };
        pkind == NodeKind::CallNode
            && self.facts.get(&(pspan, pkind)).is_some_and(|f| f.receiver_span == Some(span))
    }

    fn check_send(
        &mut self,
        span: Span,
        content: Node<'_>,
        chain: &[(Span, NodeKind)],
        ctx: &mut Context<'_>,
    ) {
        let Some(facts) = build_facts(&content, ctx) else { return };
        if facts.is_unary_operation {
            self.check_unary(span, content, chain, ctx);
            return;
        }
        if !method_call_with_redundant_parentheses(content.kind(), &facts, chain, &self.facts) {
            return;
        }
        if call_chain_starts_with_int(&content, chain, &self.facts)
            || do_end_block_in_method_chain(&content, chain, ctx)
        {
            return;
        }
        self.offense(ctx, span, "a method call");
    }

    fn check_unary(
        &mut self,
        span: Span,
        node: Node<'_>,
        chain: &[(Span, NodeKind)],
        ctx: &mut Context<'_>,
    ) {
        if self.chained(span, chain) {
            return;
        }
        let mut current = node;
        let mut guard = 0;
        loop {
            guard += 1;
            if guard > 64 {
                break;
            }
            let Some(cf) = build_facts(&current, ctx) else { break };
            if cf.is_unary_operation && !cf.is_prefix_not {
                let Some(call) = current.as_call_node() else { break };
                let Some(recv) = call.receiver() else { break };
                current = recv;
                continue;
            }
            break;
        }
        let Some(cf) = build_facts(&current, ctx) else { return };
        if !method_call_with_redundant_parentheses(current.kind(), &cf, chain, &self.facts) {
            return;
        }
        self.offense(ctx, span, "a unary operation");
    }

    fn offense(&mut self, ctx: &mut Context<'_>, span: Span, msg: &'static str) {
        self.report(
            ctx,
            span,
            msg,
            Span::new(span.start, span.start + 1),
            Span::new(span.end - 1, span.end),
        );
    }

    /// Builds and reports the fix: RuboCop's `ParenthesesCorrector.correct`
    /// -- remove `(` plus trailing whitespace/newlines, remove `)` plus
    /// leading newlines, and (for a ternary condition directly touching
    /// `?`) insert a separating space.
    fn report(&mut self, ctx: &mut Context<'_>, span: Span, msg: &str, open: Span, close: Span) {
        let message = format!("Don't use parentheses around {msg}.");
        let src = ctx.source().bytes();
        let open_end = final_pos(src, open.end, 1, true, true);
        let close_start = final_pos(src, close.start, -1, true, false).max(open_end);

        let mut edits = vec![
            Edit::delete(Span::new(open.start, open_end)),
            Edit::delete(Span::new(close_start, close.end)),
        ];

        if let Some(&(pspan, pkind)) = normalized_chain(ctx, &self.facts).first() {
            if pkind == NodeKind::IfNode {
                if let Some(f) = self.facts.get(&(pspan, pkind)) {
                    if f.is_ternary
                        && f.condition_span == Some(span)
                        && src.get(close.end as usize) == Some(&b'?')
                    {
                        edits.push(Edit::insert(close.end, b" ".to_vec()));
                    }
                }
            }
        }

        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, span, Cow::Owned(message), fix);
    }
}

fn multiline_control_flow_statements(pspan: Span, pkind: NodeKind, ctx: &Context<'_>) -> bool {
    matches!(pkind, NodeKind::ReturnNode | NodeKind::NextNode | NodeKind::BreakNode)
        && is_multiline(ctx, pspan)
}

fn is_basic_conditional(k: NodeKind) -> bool {
    matches!(k, NodeKind::IfNode | NodeKind::UnlessNode | NodeKind::WhileNode | NodeKind::UntilNode)
}

fn call_node(node: &Node<'_>) -> bool {
    node.kind() == NodeKind::CallNode
}

fn interpolation(chain: &[(Span, NodeKind)]) -> bool {
    chain.first().is_some_and(|(_, k)| *k == NodeKind::EmbeddedStatementsNode)
        && chain.get(1).is_some_and(|(_, k)| *k == NodeKind::InterpolatedStringNode)
}

fn disallowed_literal(
    content: &Node<'_>,
    span: Span,
    chain: &[(Span, NodeKind)],
    facts: &HashMap<Key, Facts>,
    ctx: &Context<'_>,
) -> bool {
    if content.kind() == NodeKind::RangeNode {
        return chain.first().is_some_and(|(_, k)| *k == NodeKind::ParenthesesNode);
    }
    !raised_to_power_negative_numeric(content, span, chain, facts, ctx)
}

/// RuboCop's `raised_to_power_negative_numeric?`: our parens is a negative
/// numeric literal used as the *base* of `**` (e.g. `(-2)**2`; but not
/// `2**(-2)`, where the parens are the exponent).
fn raised_to_power_negative_numeric(
    content: &Node<'_>,
    span: Span,
    chain: &[(Span, NodeKind)],
    facts: &HashMap<Key, Facts>,
    ctx: &Context<'_>,
) -> bool {
    if !numeric_kind(content.kind()) {
        return false;
    }
    if ctx.text(content.span()).first() != Some(&b'-') {
        return false;
    }
    let Some(&(pspan, pkind)) = chain.first() else { return false };
    pkind == NodeKind::CallNode
        && facts.get(&(pspan, pkind)).is_some_and(|f| {
            f.receiver_span == Some(span) && f.name.as_deref() == Some(b"**".as_slice())
        })
}

fn disallowed_one_line_pattern_matching(
    content_kind: NodeKind,
    chain: &[(Span, NodeKind)],
    facts: &HashMap<Key, Facts>,
) -> bool {
    if let Some(&(pspan, pkind)) = chain.first() {
        if pkind == NodeKind::DefNode && facts.get(&(pspan, pkind)).is_some_and(|f| f.is_endless) {
            return false;
        }
        if is_assignment_kind(pkind) {
            return false;
        }
    }
    matches!(content_kind, NodeKind::MatchPredicateNode | NodeKind::MatchRequiredNode)
        && !chain.iter().any(|(_, k)| matches!(k, NodeKind::AndNode | NodeKind::OrNode))
}

fn keyword_with_redundant_parentheses(
    content: &Node<'_>,
    facts: Option<&Facts>,
    ctx: &Context<'_>,
) -> bool {
    if !is_keyword(content.kind(), facts) {
        return false;
    }
    match content.kind() {
        NodeKind::BreakNode | NodeKind::NextNode | NodeKind::ReturnNode => {
            let Some(f) = facts else { return false };
            if f.args_count == 0 {
                return true;
            }
            f.args_count == 1
                && f.first_arg_span.is_some_and(|s| {
                    ctx.text(Span::new(s.start, s.start + 1)).first() == Some(&b'(')
                })
        }
        NodeKind::DefinedNode | NodeKind::SuperNode | NodeKind::YieldNode => {
            facts.is_some_and(|f| f.has_own_parens || f.args_count == 0)
        }
        NodeKind::SourceFileNode
        | NodeKind::SourceLineNode
        | NodeKind::SourceEncodingNode
        | NodeKind::SelfNode
        | NodeKind::RedoNode
        | NodeKind::RetryNode
        | NodeKind::ForwardingSuperNode => true,
        _ => false,
    }
}

fn is_lambda_or_proc_expression(content: &Node<'_>, ctx: &Context<'_>) -> bool {
    match content.kind() {
        NodeKind::LambdaNode => true,
        NodeKind::CallNode => {
            let Some(call) = content.as_call_node() else { return false };
            if !matches!(call.name().as_slice(), b"lambda" | b"proc") {
                return false;
            }
            let Some(block) = call.block() else { return false };
            let Some(block) = block.as_block_node() else { return false };
            ctx.text(block.opening_loc().span()).first() == Some(&b'{')
        }
        _ => false,
    }
}

fn method_call_with_redundant_parentheses(
    kind: NodeKind,
    facts: &Facts,
    chain: &[(Span, NodeKind)],
    ancestor_facts: &HashMap<Key, Facts>,
) -> bool {
    if !matches!(
        kind,
        NodeKind::CallNode
            | NodeKind::SuperNode
            | NodeKind::ForwardingSuperNode
            | NodeKind::YieldNode
            | NodeKind::DefinedNode
    ) {
        return false;
    }
    if facts.is_prefix_not {
        return false;
    }
    if singular_parenthesized_parent(chain, ancestor_facts) {
        return true;
    }
    facts.args_count == 0
        || facts.has_own_parens
        || square_brackets(kind, facts, chain, ancestor_facts)
}

fn singular_parenthesized_parent(
    chain: &[(Span, NodeKind)],
    ancestor_facts: &HashMap<Key, Facts>,
) -> bool {
    let Some(&(pspan, pkind)) = chain.first() else { return true };
    if matches!(pkind, NodeKind::SplatNode | NodeKind::AssocSplatNode) {
        return false;
    }
    match pkind {
        NodeKind::ReturnNode | NodeKind::NextNode | NodeKind::BreakNode => {
            ancestor_facts.get(&(pspan, pkind)).is_some_and(|f| f.args_count == 1)
        }
        NodeKind::ArrayNode => {
            ancestor_facts.get(&(pspan, pkind)).is_some_and(|f| f.elements_count == 1)
        }
        _ => false,
    }
}

/// RuboCop's `square_brackets?` matcher: `recv.method[...]`/`str[...]`/
/// `array[...]`/`hash[...]`/`const[...]`/`var[...]` -- our own parens is
/// the receiver of an immediately-following `[]`/`[]=` call.
fn square_brackets(
    kind: NodeKind,
    _facts: &Facts,
    chain: &[(Span, NodeKind)],
    ancestor_facts: &HashMap<Key, Facts>,
) -> bool {
    if kind != NodeKind::CallNode {
        return false;
    }
    let Some(&(pspan, pkind)) = chain.first() else { return false };
    if pkind != NodeKind::CallNode {
        return false;
    }
    ancestor_facts
        .get(&(pspan, pkind))
        .is_some_and(|f| matches!(f.name.as_deref(), Some(b"[]" | b"[]=")))
}
