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
//! # Correction verification (new in 1.91.0)
//!
//! The cop no longer decides redundancy from hand-maintained grammar
//! knowledge alone: every candidate is held back until
//! `on_investigation_end`, where the `ReparsedEquivalence` mixin applies
//! the candidate's exact correction, reparses, and keeps the offense only
//! when the tree is unchanged. That replaced the deleted
//! `first_arg_begins_with_hash_literal?`, `raised_to_power_negative_numeric?`,
//! `call_chain_starts_with_int?` and `do_end_block_in_method_chain?`
//! guards. [`Shape`] is the Rust side of that comparison: the correction
//! only ever deletes `(`, `)` and adjacent whitespace, so both token
//! streams are identical by construction and any difference can only be
//! structural -- comparing node kinds and child structure is therefore
//! enough. It also makes upstream's `preprocess_reparsed_source` hook
//! unnecessary: whitequark bakes `__LINE__`/`__FILE__` into literal
//! values, while Prism keeps them as valueless nodes.
//!
//! # Blind spots (documented in `META.blind_spots`)
//! One-line-rescue and multi-statement-parens edge cases are approximated
//! in the direction of false negatives, and
//! `singular_parenthesized_parent?`'s generic `parent.children.one?` is
//! only evaluated for the parent kinds that can actually hold a single
//! child here. `Style/ParenthesesAroundCondition`'s
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
use ruby_ast::{LocationExt, Node, NodeExt, NodeKind, Parsed};
use ruby_source::{Side, SourceFile, Span};

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
    /// `StatementsNode`/`ParenthesesNode`: number of statements in the
    /// body -- whitequark's child count for a `begin` node.
    stmt_count: usize,
    /// `StatementsNode`/`ParenthesesNode`: the first statement's span,
    /// for `body_range?`'s `parent.children.first` identity check.
    first_child_span: Option<Span>,
    /// `StatementsNode`/`ParenthesesNode`: the last statement's span, for
    /// `body_range?`'s `parent.children.last` identity check and for the
    /// corrector's `node.children.last`.
    last_child_span: Option<Span>,
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

/// RuboCop-AST's `Node#multiline?` for a node's own span.
fn is_multiline(ctx: &Context<'_>, span: Span) -> bool {
    !ctx.is_single_line(span)
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
            let body = node.as_statements_node()?.body();
            f.stmt_count = body.len();
            f.first_child_span = body.iter().next().map(|c| c.span());
            f.last_child_span = body.iter().last().map(|c| c.span());
        }
        NodeKind::ParenthesesNode => {
            let body = node.as_parentheses_node()?.body();
            if let Some(stmts) = body.as_ref().and_then(Node::as_statements_node) {
                let body = stmts.body();
                f.stmt_count = body.len();
                f.first_child_span = body.iter().next().map(|c| c.span());
                f.last_child_span = body.iter().last().map(|c| c.span());
            }
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

/// One offense candidate. Upstream parks these in `@pending_offenses`
/// until `on_investigation_end` verifies each correction by reparsing.
#[derive(Debug, Clone)]
struct Pending {
    /// The reported span: the parentheses being removed.
    span: Span,
    /// `ReparsedEquivalence#reparse_scope`: the innermost `def`/`class`/
    /// `module`/`class << self` containing the candidate, which parses
    /// standalone; `None` means the whole file.
    scope: Option<Span>,
    message: &'static str,
    /// `ParenthesesCorrector.correct`'s edits -- applied to verify, and
    /// again as the published fix once verified.
    edits: Vec<Edit>,
}

/// Looks for redundant parentheses.
#[derive(Debug, Clone, Default)]
pub struct RedundantParentheses {
    facts: HashMap<Key, Facts>,
    /// Candidates awaiting reparse verification in `file_end`.
    pending: Vec<Pending>,
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
        stability: Stability::Stable,
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
ancestor-walk guard for nested cases. `singular_parenthesized_parent?`'s \
`parent.children.one?` is only evaluated for `return`/`next`/`break` and \
array parents.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let ternary_enabled = options
            .peer("Style/TernaryParentheses", "Enabled")
            .and_then(linter::OptionValue::as_bool)
            .unwrap_or(true);
        let style = options
            .peer("Style/TernaryParentheses", "EnforcedStyle")
            .and_then(|v| v.as_str())
            .map_or_else(|| "require_no_parentheses".to_string(), str::to_string);
        let parens_around_condition_enabled = options
            .peer("Style/ParenthesesAroundCondition", "Enabled")
            .and_then(linter::OptionValue::as_bool)
            .unwrap_or(true);
        let allow_in_multiline_conditions = parens_around_condition_enabled
            && options
                .peer("Style/ParenthesesAroundCondition", "AllowInMultilineConditions")
                .and_then(linter::OptionValue::as_bool)
                .unwrap_or(false);
        Ok(Self {
            facts: HashMap::new(),
            pending: Vec::new(),
            ternary_parentheses_required: ternary_enabled
                && matches!(
                    style.as_str(),
                    "require_parentheses" | "require_parentheses_when_complex"
                ),
            allow_in_multiline_conditions,
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.facts.clear();
        self.pending.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(f) = build_facts(node, ctx) {
            self.facts.insert((node.span(), node.kind()), f);
        }
        match node.kind() {
            NodeKind::ParenthesesNode => self.check_parens(node, ctx),
            NodeKind::PinnedExpressionNode => self.check_pin(node, ctx),
            _ => {}
        }
    }

    /// RuboCop's `on_investigation_end`: every parked candidate whose exact
    /// correction reparses to the same tree becomes an offense.
    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let pending = std::mem::take(&mut self.pending);
        let verified = verify_by_reparse(&pending, ctx);
        for (item, ok) in pending.into_iter().zip(verified) {
            if !ok {
                continue;
            }
            let message = format!("Don't use parentheses around {}.", item.message);
            let fix = Fix { applicability: Applicability::Safe, edits: item.edits };
            ctx.report_with_fix(&Self::META, item.span, Cow::Owned(message), fix);
        }
    }
}

impl RedundantParentheses {
    fn check_pin(&mut self, node: &Node<'_>, ctx: &Context<'_>) {
        let Some(pin) = node.as_pinned_expression_node() else { return };
        let lparen = pin.lparen_loc();
        let rparen = pin.rparen_loc();
        let inner = pin.expression();
        if !variable_kind(inner.kind()) {
            return;
        }
        let span = Span::new(lparen.span().start, rparen.span().end);
        let body_end = inner.span().end;
        self.record(ctx, span, body_end, "a variable", lparen.span(), rparen.span(), None);
    }

    fn check_parens(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(parens) = node.as_parentheses_node() else { return };
        let span = node.span();
        let Some(body) = parens.body() else { return }; // `()`
        let Some(stmts) = body.as_statements_node() else { return };
        let list = stmts.body();
        let Some(content) = list.iter().next() else { return };
        // `ParenthesesCorrector`'s `node.children.last`.
        let body_end = list.iter().last().map_or(span.end, |n| n.span().end);

        let chain = normalized_chain(ctx, &self.facts);

        if self.parens_allowed(&content, span, &list, &chain, ctx) {
            return;
        }
        if self.ignore_syntax(span, &chain, ctx) {
            return;
        }

        self.check(span, body_end, content, &chain, ctx);
    }

    fn parens_allowed(
        &self,
        content: &Node<'_>,
        span: Span,
        list: &NodeList<'_>,
        chain: &[(Span, NodeKind)],
        ctx: &Context<'_>,
    ) -> bool {
        Self::rescue_ancestor(chain)
            || Self::in_pattern_matching_in_method_argument(content.kind(), chain)
            || self.allowed_expression(content, span, list, chain, ctx)
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
        body_end: u32,
        content: Node<'_>,
        chain: &[(Span, NodeKind)],
        ctx: &mut Context<'_>,
    ) {
        if let Some(msg) = self.find_offense_message(span, &content, chain, ctx) {
            if msg == BLOCK_BODY {
                self.offense_at(ctx, span, body_end, msg, None);
                return;
            }
            let mut report_span = span;
            let mut report_body_end = body_end;
            if content.kind() == NodeKind::RangeNode
                && !self.argument_of_parenthesized_method_call(
                    span,
                    content.kind(),
                    chain,
                    build_facts(&content, ctx).as_ref(),
                )
            {
                if let Some(&key) = chain.first() {
                    if key.1 == NodeKind::ParenthesesNode {
                        report_span = key.0;
                        if let Some(last) = self.facts.get(&key).and_then(|f| f.last_child_span) {
                            report_body_end = last.end;
                        }
                    }
                }
            }
            let heredoc_end = heredoc_end(&content, ctx);
            self.offense_at(ctx, report_span, report_body_end, msg, heredoc_end);
            return;
        }
        if call_node(&content) {
            self.check_send(span, body_end, content, chain, ctx);
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
        if literal_kind(kind) && disallowed_literal(content, chain, &self.facts) {
            return Some("a literal");
        }
        if variable_kind(kind) {
            return Some("a variable");
        }
        if const_kind(kind) {
            return Some("a constant");
        }
        if chain.first().is_some_and(|&(_, k)| is_block_kind(k))
            || self.body_range(span, content, chain)
        {
            return Some(BLOCK_BODY);
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
        if self.argument_of_parenthesized_method_call(span, kind, chain, content_facts.as_ref())
            && !keyword_logical_operator(kind, content_facts.as_ref())
        {
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
        body_end: u32,
        content: Node<'_>,
        chain: &[(Span, NodeKind)],
        ctx: &mut Context<'_>,
    ) {
        let Some(facts) = build_facts(&content, ctx) else { return };
        if facts.is_unary_operation {
            self.check_unary(span, body_end, content, chain, ctx);
            return;
        }
        if !method_call_with_redundant_parentheses(&content, &facts, chain, &self.facts) {
            return;
        }
        self.offense(ctx, span, body_end, "a method call");
    }

    fn check_unary(
        &mut self,
        span: Span,
        body_end: u32,
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
        if !method_call_with_redundant_parentheses(&current, &cf, chain, &self.facts) {
            return;
        }
        self.offense(ctx, span, body_end, "a unary operation");
    }

    /// RuboCop's `body_range?`: an open-ended range that is the first or
    /// last statement of a `begin`, where the parentheses only delimit the
    /// statement rather than the range's missing end.
    fn body_range(&self, span: Span, content: &Node<'_>, chain: &[(Span, NodeKind)]) -> bool {
        if self.chained(span, chain) {
            return false;
        }
        let Some(range) = content.as_range_node() else { return false };
        let Some(&key) = chain.first() else { return false };
        if !matches!(key.1, NodeKind::ParenthesesNode | NodeKind::StatementsNode) {
            return false;
        }
        let Some(f) = self.facts.get(&key) else { return false };
        (range.left().is_none() && f.first_child_span == Some(span))
            || (range.right().is_none() && f.last_child_span == Some(span))
    }

    fn offense(&mut self, ctx: &Context<'_>, span: Span, body_end: u32, msg: &'static str) {
        self.offense_at(ctx, span, body_end, msg, None);
    }

    fn offense_at(
        &mut self,
        ctx: &Context<'_>,
        span: Span,
        body_end: u32,
        msg: &'static str,
        heredoc_end: Option<u32>,
    ) {
        self.record(
            ctx,
            span,
            body_end,
            msg,
            Span::new(span.start, span.start + 1),
            Span::new(span.end - 1, span.end),
            heredoc_end,
        );
    }

    /// RuboCop's `offense`: parks the candidate together with its exact
    /// correction -- `ParenthesesCorrector.correct`, i.e. remove `(` plus
    /// trailing whitespace/newlines, remove `)` plus leading whitespace
    /// (and the newline, unless that would pull a chained call into the
    /// comment on the line above), and (for a ternary condition directly
    /// touching `?`) insert a separating space.
    #[allow(clippy::too_many_arguments)]
    fn record(
        &mut self,
        ctx: &Context<'_>,
        span: Span,
        body_end: u32,
        msg: &'static str,
        open: Span,
        close: Span,
        heredoc_end: Option<u32>,
    ) {
        let src = ctx.source().bytes();
        let open_end =
            ctx.with_surrounding_space(Span::new(open.end, open.end), Side::Right, true, true).end;
        let newlines = !comment_above_close_paren_swallows_chain(ctx, body_end, close);
        let close_start = ctx
            .with_surrounding_space(
                Span::new(close.start, close.start),
                Side::Left,
                newlines,
                false,
            )
            .start
            .max(open_end);

        let mut edits = vec![Edit::delete(Span::new(open.start, open_end))];

        let mut close_end = close.end;
        if let Some(insert) = heredoc_end {
            if only_closing_paren_before_comma(ctx, close.start) {
                let mut comma_end = close.end;
                while (comma_end as usize) < src.len()
                    && matches!(src[comma_end as usize], b' ' | b'\t')
                {
                    comma_end += 1;
                }
                if src.get(comma_end as usize) == Some(&b',') {
                    close_end = comma_end + 1;
                    edits.push(Edit::insert(insert, b",".to_vec()));
                }
            }
        }
        edits.push(Edit::delete(Span::new(close_start, close_end)));

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

        self.pending.push(Pending { span, scope: reparse_scope(ctx), message: msg, edits });
    }
}

/// RuboCop's `only_closing_paren_before_comma?`: the whole line containing
/// `close_paren_pos` (a `)`), from column 0, matches `/\A\s*\)\s*,/`.
fn only_closing_paren_before_comma(ctx: &Context<'_>, close_paren_pos: u32) -> bool {
    let line = ctx.line_col(close_paren_pos).line;
    let text = ctx.line_text(line);
    let mut i = 0;
    while i < text.len() && matches!(text[i], b' ' | b'\t') {
        i += 1;
    }
    if text.get(i) != Some(&b')') {
        return false;
    }
    i += 1;
    while i < text.len() && matches!(text[i], b' ' | b'\t') {
        i += 1;
    }
    text.get(i) == Some(&b',')
}

/// The end offset of a heredoc-opening string literal (`StringNode`-family
/// whose opening delimiter is `<<...`), for `add_heredoc_comma`'s
/// insertion point.
fn heredoc_end(content: &Node<'_>, ctx: &Context<'_>) -> Option<u32> {
    let opening = match content.kind() {
        NodeKind::StringNode => content.as_string_node()?.opening_loc(),
        NodeKind::InterpolatedStringNode => content.as_interpolated_string_node()?.opening_loc(),
        NodeKind::XStringNode => Some(content.as_x_string_node()?.opening_loc()),
        NodeKind::InterpolatedXStringNode => {
            Some(content.as_interpolated_x_string_node()?.opening_loc())
        }
        _ => return None,
    }?;
    if ctx.text(opening.span()).starts_with(b"<<") {
        Some(content.span().end)
    } else {
        None
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

/// RuboCop's `disallowed_literal?`: every literal but a range is
/// disallowed outright; a range only when its parentheses are the whole
/// of a single-child `begin`.
fn disallowed_literal(
    content: &Node<'_>,
    chain: &[(Span, NodeKind)],
    facts: &HashMap<Key, Facts>,
) -> bool {
    if content.kind() != NodeKind::RangeNode {
        return true;
    }
    let Some(&key) = chain.first() else { return false };
    matches!(key.1, NodeKind::ParenthesesNode | NodeKind::StatementsNode)
        && facts.get(&key).is_some_and(|f| f.stmt_count == 1)
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
    content: &Node<'_>,
    facts: &Facts,
    chain: &[(Span, NodeKind)],
    ancestor_facts: &HashMap<Key, Facts>,
) -> bool {
    if !matches!(
        content.kind(),
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
    facts.args_count == 0 || facts.has_own_parens || square_brackets(content)
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

/// RuboCop's `square_brackets?` matcher: `content` is itself
/// `recv.method[...]`/`str[...]`/`array[...]`/`hash[...]`/`const[...]`/
/// `var[...]` -- an *existing* subscript expression whose receiver chain
/// bottoms out in something safe to unwrap (a literal, a constant, a
/// variable, or a zero-arg call). Unlike the old (backwards) version, this
/// looks at our own content node, not at whatever our parens' *parent*
/// happens to be -- `square_brackets?` is applied to the node being
/// checked for redundant parens, never to an ancestor.
fn square_brackets(content: &Node<'_>) -> bool {
    let Some(call) = content.as_call_node() else { return false };
    if call.name().as_slice() != b"[]" {
        return false;
    }
    call.receiver().is_some_and(|r| square_brackets_operand(&r))
}

/// The node pattern's `` `{(send _recv _msg) str array hash const
/// #variable?} `` alternation: descends through a bare receiver chain
/// (mirroring the pattern's backtick search) looking for a literal,
/// constant, variable, or zero-argument call to bottom out on.
fn square_brackets_operand(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::StringNode | NodeKind::ArrayNode | NodeKind::HashNode => true,
        k if const_kind(k) || variable_kind(k) => true,
        NodeKind::CallNode => {
            let Some(call) = node.as_call_node() else { return false };
            let args_count = call.arguments().as_ref().map_or(0, |a| a.arguments().len());
            if args_count == 0 {
                return true;
            }
            call.receiver().is_some_and(|r| square_brackets_operand(&r))
        }
        _ => false,
    }
}

/// The `'block body'` message fragment, which [`RedundantParentheses::check`]
/// also uses as upstream does: as the marker for the early-return path
/// that skips the range-literal parent adjustment.
const BLOCK_BODY: &str = "block body";

/// RuboCop-AST's `any_block_type?` (`block`/`numblock`/`itblock`): Prism's
/// `BlockNode`, plus `LambdaNode`, which whitequark also models as a
/// `block` node wrapping the `lambda` send.
fn is_block_kind(k: NodeKind) -> bool {
    matches!(k, NodeKind::BlockNode | NodeKind::LambdaNode)
}

/// RuboCop's `keyword_logical_operator?`: `and`/`or` bind looser than the
/// method-argument boundary, so `foo((x and y))` cannot drop its
/// parentheses without becoming a syntax error (unlike `&&`/`||`).
fn keyword_logical_operator(kind: NodeKind, facts: Option<&Facts>) -> bool {
    matches!(kind, NodeKind::AndNode | NodeKind::OrNode)
        && facts.is_some_and(|f| f.is_semantic_operator)
}

/// `ParenthesesCorrector#comment_above_close_paren_swallows_chain?`: when
/// the line above `)` ends in a comment and a chained call follows `)`,
/// removing the newline along with `)` would pull the chain into the
/// comment.
fn comment_above_close_paren_swallows_chain(ctx: &Context<'_>, body_end: u32, close: Span) -> bool {
    if body_end >= close.start {
        return false;
    }
    if !has_comment_line(ctx.text(Span::new(body_end, close.start))) {
        return false;
    }
    chained_after_close_paren(ctx, close)
}

/// The corrector's `/#[^\n]*\n/` match: a `#` with a newline after it.
/// Only the first `#` needs looking at -- any later one is followed by the
/// same newlines.
fn has_comment_line(between: &[u8]) -> bool {
    between
        .iter()
        .position(|&b| b == b'#')
        .and_then(|hash| between.get(hash + 1..))
        .is_some_and(|rest| rest.contains(&b'\n'))
}

/// `ParenthesesCorrector#chained_after_close_paren?`: something other than
/// a comment follows `)` on its line.
fn chained_after_close_paren(ctx: &Context<'_>, close: Span) -> bool {
    let bytes = ctx.source().bytes();
    let Some(rest) = bytes.get(close.range().end..) else { return false };
    let line = rest.iter().position(|&b| b == b'\n').map_or(rest, |i| &rest[..i]);
    match line.iter().position(|b| !b.is_ascii_whitespace()) {
        Some(i) => line.get(i) != Some(&b'#'),
        None => false,
    }
}

/// `ReparsedEquivalence::MAX_VERIFICATION_FRAGMENT_SIZE`: above this, a
/// scope is accepted unverified (upstream's `oversized: :report`).
const MAX_VERIFICATION_FRAGMENT_SIZE: usize = 64 * 1024;

/// `ReparsedEquivalence#reparse_scope`: the innermost scope containing the
/// candidate that parses standalone. Method definitions and class/module
/// bodies neither capture outer local variables nor continue an outer
/// expression; blocks and single statements do not qualify.
fn reparse_scope(ctx: &Context<'_>) -> Option<Span> {
    ctx.ancestors()
        .iter()
        .rev()
        .find(|info| {
            matches!(
                info.kind,
                NodeKind::DefNode
                    | NodeKind::ClassNode
                    | NodeKind::ModuleNode
                    | NodeKind::SingletonClassNode
            )
        })
        .map(|info| info.span)
}

/// A Prism tree reduced to what upstream's `normalize_reparsed_ast`
/// compares: node kinds and child structure, with grouping parentheses
/// and statement sequences transparent (a single-statement sequence
/// collapses into its statement, a nested sequence is spliced in place)
/// and chains of the same `&&`/`||` operator left-rotated. See the module
/// doc for why kinds and structure alone settle the comparison.
#[derive(Debug, PartialEq, Eq)]
struct Shape {
    kind: NodeKind,
    children: Vec<Shape>,
}

impl Shape {
    fn of(node: &Node<'_>) -> Self {
        let kind = match node.kind() {
            // `^(x)` and `^x` are one `pin` node with one child upstream;
            // Prism splits them into two node kinds.
            NodeKind::PinnedExpressionNode => NodeKind::PinnedVariableNode,
            other => other,
        };
        let mut children = Vec::new();
        ruby_ast::for_each_child(node, |child| children.push(Self::of(child)));
        match kind {
            // Prism-only wrappers: whitequark reads straight through to
            // the statement sequence.
            NodeKind::ProgramNode | NodeKind::ParenthesesNode => {
                if children.len() == 1 {
                    children.remove(0)
                } else {
                    Self { kind: NodeKind::StatementsNode, children }
                }
            }
            NodeKind::StatementsNode => {
                let mut spliced = splice_nested_sequences(children);
                if spliced.len() == 1 {
                    spliced.remove(0)
                } else {
                    Self { kind, children: spliced }
                }
            }
            _ => rotate_same_operator(Self { kind, children }),
        }
    }
}

/// Upstream's `splice_nested_sequences`: a statement sequence nested
/// directly in another one contributes its statements in place.
fn splice_nested_sequences(children: Vec<Shape>) -> Vec<Shape> {
    if !children.iter().any(|c| c.kind == NodeKind::StatementsNode) {
        return children;
    }
    let mut out = Vec::with_capacity(children.len());
    for child in children {
        if child.kind == NodeKind::StatementsNode {
            out.extend(child.children);
        } else {
            out.push(child);
        }
    }
    out
}

/// Upstream's `rotate_same_operator`: `x && (y && z)` and `x && y && z`
/// differ as trees, but `&&`/`||` cannot be redefined, so same-operator
/// regrouping is semantically transparent.
fn rotate_same_operator(shape: Shape) -> Shape {
    if !matches!(shape.kind, NodeKind::AndNode | NodeKind::OrNode) {
        return shape;
    }
    let kind = shape.kind;
    let mut children = shape.children;
    if children.len() != 2 || children[1].kind != kind || children[1].children.len() != 2 {
        return Shape { kind, children };
    }
    let mut right = children.remove(1);
    let lhs = children.remove(0);
    let right_rhs = right.children.remove(1);
    let right_lhs = right.children.remove(0);
    let rotated_left = rotate_same_operator(Shape { kind, children: vec![lhs, right_lhs] });
    rotate_same_operator(Shape { kind, children: vec![rotated_left, right_rhs] })
}

/// Parses `fragment` standalone and reduces it to a [`Shape`]; `None` when
/// it doesn't parse, upstream's `valid_syntax?`.
fn parse_shape(fragment: &[u8]) -> Option<Shape> {
    let source = SourceFile::new("(redundant_parentheses)", fragment.to_vec());
    let parsed = Parsed::parse(&source);
    if parsed.has_errors() {
        return None;
    }
    Some(Shape::of(&parsed.root()))
}

/// Applies `edits`, which must be in source order, to `original`; `None`
/// when two of them overlap -- upstream's rescued `ClobberingError`.
fn apply_edits(original: &[u8], edits: &[Edit]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(original.len());
    let mut pos = 0usize;
    for edit in edits {
        let range = edit.span.range();
        if range.start < pos || range.end > original.len() {
            return None;
        }
        out.extend_from_slice(original.get(pos..range.start)?);
        out.extend_from_slice(&edit.replacement);
        pos = range.end;
    }
    out.extend_from_slice(original.get(pos..)?);
    Some(out)
}

/// `ReparsedEquivalence#corrections_verify?`: applies every listed
/// candidate's exact correction at once and checks that the scope still
/// parses to the same tree.
fn corrections_verify(
    src: &[u8],
    scope: Option<Span>,
    items: &[usize],
    pending: &[Pending],
    original: &Shape,
) -> bool {
    let mut edits: Vec<Edit> =
        items.iter().flat_map(|&i| pending[i].edits.iter().cloned()).collect();
    edits.sort_by_key(|e| e.span.start);
    let Some(corrected) = apply_edits(src, &edits) else { return false };
    let fragment = match scope {
        // Every edit is inside the scope, so the corrected fragment is the
        // scope's range shifted by the edits' total length delta.
        Some(s) => {
            let end = s.range().end.checked_add(corrected.len());
            let Some(end) = end.and_then(|e| e.checked_sub(src.len())) else { return false };
            let Some(fragment) = corrected.get(s.range().start..end) else { return false };
            fragment
        }
        None => &corrected[..],
    };
    parse_shape(fragment).is_some_and(|shape| shape == *original)
}

/// `ReparsedEquivalence#verified_by_reparse` with the default
/// `oversized: :report`: candidates are grouped by reparse scope, a whole
/// group is taken when its combined correction reparses equivalently, and
/// otherwise every candidate is verified on its own.
fn verify_by_reparse(pending: &[Pending], ctx: &Context<'_>) -> Vec<bool> {
    let mut verified = vec![false; pending.len()];
    if pending.is_empty() {
        return verified;
    }
    let src = ctx.source().bytes();
    let mut scopes: Vec<Option<Span>> = Vec::new();
    for item in pending {
        if !scopes.contains(&item.scope) {
            scopes.push(item.scope);
        }
    }
    for scope in scopes {
        let group: Vec<usize> = (0..pending.len()).filter(|&i| pending[i].scope == scope).collect();
        if scope.map_or(src.len(), |s| s.range().len()) > MAX_VERIFICATION_FRAGMENT_SIZE {
            for i in group {
                verified[i] = true;
            }
            continue;
        }
        // A scope that doesn't parse standalone falls back to the whole
        // file, which upstream compares against the live AST instead.
        let mut scope = scope;
        let mut original = scope.and_then(|s| src.get(s.range())).and_then(parse_shape);
        if original.is_none() {
            scope = None;
            original = parse_shape(src);
        }
        let Some(original) = original else { continue };
        if group.len() > 1 && corrections_verify(src, scope, &group, pending, &original) {
            for i in group {
                verified[i] = true;
            }
        } else {
            for i in group {
                verified[i] = corrections_verify(src, scope, &[i], pending, &original);
            }
        }
    }
    verified
}
