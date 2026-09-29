//! `Style/ParenthesesAroundCondition`, ported from RuboCop's
//! `lib/rubocop/cop/style/parentheses_around_condition.rb` plus the
//! `Parentheses` and `SafeAssignment` mixins it includes, and the
//! `ParenthesesCorrector` its autocorrection uses.
//!
//! # Node shapes
//!
//! Whitequark's single `:if` node type covers `if`, `elsif`, ternaries and
//! `unless`, distinguished by `keyword`/`ternary?`; Prism splits it into
//! [`NodeKind::IfNode`] (`if`/`elsif`/ternary) and its own
//! [`NodeKind::UnlessNode`]. An `elsif` is a nested `IfNode`, distinguished
//! from a genuine `if` only by `if_keyword_loc` reading `"elsif"` (5 bytes)
//! rather than `"if"` (2 bytes) -- see `Lint/DuplicateElsifCondition`'s
//! module doc for the same shape -- and a ternary has no `if_keyword_loc` at
//! all. `on_while`/`on_until` (aliased upstream) become the two separate
//! kinds [`NodeKind::WhileNode`]/[`NodeKind::UntilNode`] here.
//!
//! Whitequark's `begin` node (any parenthesized/grouped expression) is
//! Prism's [`ruby_ast::node::ParenthesesNode`]; its single wrapped
//! statement, or several separated by `;`, always sits inside an
//! intervening `StatementsNode` (unlike whitequark, which elides that
//! wrapper for a single statement). `control_op_condition`'s `(begin $_
//! $...)` pattern -- which never matches an empty `()`, whose `body` is
//! simply absent -- is reproduced by requiring a `body` and reading its
//! `StatementsNode`'s elements directly.
//!
//! `modifier_op?`'s `node.rescue_type?` is Prism's
//! [`NodeKind::RescueModifierNode`] (see `Style/RescueModifier`'s module
//! doc); its `node.basic_conditional? && node.modifier_form?` (whitequark
//! types `:if`/`:while`/`:until`, excluding `:while_post`/`:until_post`,
//! with no `loc.end`) is `IfNode`/`UnlessNode`/`WhileNode`/`UntilNode`
//! (excluding a `WhileNode`/`UntilNode` with `is_begin_modifier()` set, i.e.
//! `begin ... end while`/`until`) whose own `end_keyword_loc`/`closing_loc`
//! is absent.
//!
//! `require_parentheses?`'s `condition_body.send_node.block_literal?` is,
//! for any block-carrying node, trivially true (`block_literal?` checks
//! that the call is its own attached block's call, which it always is), so
//! it collapses to just `condition_body.any_block_type? &&
//! condition_body.keywords?` -- a `CallNode` whose attached block (reached
//! through `as_block_node`, since `CallNode::block` can also yield a
//! `BlockArgumentNode` for `&blk`, per the crate-wide Prism trap) closes
//! with `end` rather than `}` (`BlockNode::closing_loc`'s length: 3 for
//! `"end"`, 1 for `"}"`).
//!
//! # Autocorrection
//!
//! `ParenthesesCorrector`'s comment-above-close-paren/chained-call
//! preservation, ternary spacing (dead here: a ternary condition is never
//! flagged in the first place, since [`Rule::enter`] returns before
//! `check` for one), and orphaned-trailing-comma handling are not
//! reproduced -- only plain paren removal is ported (matching
//! `Style/RescueModifier`'s own `ParenthesesCorrector` port), since no
//! fixture exercises those cases. Removal itself is fully faithful: the
//! opening paren is removed together with its trailing space/tab, then any
//! run of bare newlines, then any further run of Ruby whitespace (crossing
//! blank lines and leading indentation, `range_with_surrounding_space(side:
//! :right, whitespace: true)`); the closing paren is removed together with
//! only a leading run of bare newlines (`side: :left`, `whitespace`
//! defaulted `false`).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::ParenthesesNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// Don't use parentheses around the condition of an if/unless/while.
#[derive(Debug, Clone)]
pub struct ParenthesesAroundCondition {
    /// `AllowInMultilineConditions`.
    allow_multiline_conditions: bool,
    /// `safe_assignment_kept?`: this cop's own `AllowSafeAssignment`, or
    /// `Lint/AssignmentInCondition`'s own `AllowSafeAssignment` (read as a
    /// peer option; `true` when unset, matching its `default.yml` default
    /// -- see [`Rule::configure`]).
    safe_assignment_kept: bool,
}

impl Rule for ParenthesesAroundCondition {
    const META: RuleMeta = RuleMeta {
        name: "Style/ParenthesesAroundCondition",
        department: Department::Style,
        summary: "Don't use parentheses around the condition of an if/unless/while.",
        explanation: "`AllowSafeAssignment` option for safe assignment. By safe assignment we \
mean putting parentheses around an assignment to indicate \"I know I'm using an assignment as a \
condition. It's not a mistake.\"",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode, NodeKind::WhileNode, NodeKind::UntilNode],
        config: &[
            ConfigOption {
                name: "AllowSafeAssignment",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether an assignment wrapped in parentheses (e.g. `if (test = 10)`) is \
allowed.",
            },
            ConfigOption {
                name: "AllowInMultilineConditions",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether parentheses are allowed around a multiline condition.",
            },
        ],
        blind_spots: "`ParenthesesCorrector`'s comment-above-close-paren/chained-call \
preservation and orphaned-trailing-comma handling are not reproduced, since no fixture exercises \
those cases.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let allow_safe_assignment = options.bool("AllowSafeAssignment");
        // `config.for_enabled_cop('Lint/AssignmentInCondition')['AllowSafeAssignment']`,
        // read as a peer option; `true` when unconfigured, matching
        // `Lint/AssignmentInCondition`'s own `default.yml` default (this
        // engine's peer table always carries every cop's own `default.yml`
        // defaults, unlike upstream's per-cop RSpec unit-test `:config`
        // context, which starts from an isolated `{AllCops, <cop under
        // test>}` hash with no other cop's entry at all -- see the two
        // deleted fixture cases this cop's directory no longer has).
        let peer_allow_safe_assignment = options
            .peer("Lint/AssignmentInCondition", "AllowSafeAssignment")
            .and_then(OptionValue::as_bool)
            .unwrap_or(true);
        Ok(Self {
            allow_multiline_conditions: options.bool("AllowInMultilineConditions"),
            safe_assignment_kept: allow_safe_assignment || peer_allow_safe_assignment,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (condition, keyword) = match node.kind() {
            NodeKind::IfNode => {
                let n = node.as_if_node().expect("kind matched");
                let Some(if_kw) = n.if_keyword_loc() else { return }; // ternary: no keyword at all.
                let keyword = if if_kw.span().len() == 5 { "elsif" } else { "if" };
                (n.predicate(), keyword)
            }
            NodeKind::UnlessNode => {
                (node.as_unless_node().expect("kind matched").predicate(), "unless")
            }
            NodeKind::WhileNode => {
                (node.as_while_node().expect("kind matched").predicate(), "while")
            }
            NodeKind::UntilNode => {
                (node.as_until_node().expect("kind matched").predicate(), "until")
            }
            _ => return,
        };
        self.check(&condition, keyword, ctx);
    }
}

impl ParenthesesAroundCondition {
    /// RuboCop's `process_control_op` plus its `control_op_condition`
    /// matcher.
    fn check(&self, condition: &Node<'_>, keyword: &'static str, ctx: &mut Context<'_>) {
        let Some(paren) = condition.as_parentheses_node() else { return };
        // `(begin)` (an empty `()`) never matches `control_op_condition`'s
        // `(begin $_ $...)`, which requires at least one child.
        let Some(body) = paren.body() else { return };
        let Some(stmts) = body.as_statements_node() else { return };
        let list = stmts.body();
        let Some(first_child) = list.first() else { return };

        if requires_parens_for_block(keyword, &first_child) {
            return;
        }
        if let Some(second_child) = list.iter().nth(1) {
            let between = Span::new(first_child.span().end, second_child.span().start);
            if ctx.text(between).contains(&b';') {
                return;
            }
        }
        if is_modifier_op(&first_child) {
            return;
        }
        if self.parens_allowed(ctx, &paren, condition) {
            return;
        }

        let article = if keyword == "while" { "a" } else { "an" };
        let message =
            format!("Don't use parentheses around the condition of {article} `{keyword}`.");
        let fix = Fix { applicability: Applicability::Safe, edits: remove_parens(ctx, &paren) };
        ctx.report_with_fix(&Self::META, condition.span(), message, fix);
    }

    /// RuboCop's `parens_allowed?`.
    fn parens_allowed(
        &self,
        ctx: &Context<'_>,
        paren: &ParenthesesNode<'_>,
        condition: &Node<'_>,
    ) -> bool {
        parens_required(ctx, condition.span())
            || (is_safe_assignment(paren) && self.safe_assignment_kept)
            || (!ctx.is_single_line(condition.span()) && self.allow_multiline_conditions)
    }
}

/// RuboCop's `Parentheses#parens_required?`: true when the character
/// immediately before the condition's opening paren, or immediately after
/// its closing paren, is a lowercase ASCII letter -- i.e. there is no space
/// between the keyword and `(` (`if(x > 5)`), or between `)` and whatever
/// follows.
fn parens_required(ctx: &Context<'_>, span: Span) -> bool {
    let byte_at = |offset: u32| ctx.source().bytes().get(offset as usize).copied();
    let before = span.start.checked_sub(1).and_then(byte_at);
    let after = byte_at(span.end);
    before.is_some_and(|b| b.is_ascii_lowercase()) || after.is_some_and(|b| b.is_ascii_lowercase())
}

/// RuboCop's `require_parentheses?`: only for `while`/`until`, when the
/// condition is itself a block call closed with `do`...`end` (never for
/// `{`...`}`, which parses tighter than the loop keyword and so needs no
/// protecting parens).
fn requires_parens_for_block(keyword: &str, first_child: &Node<'_>) -> bool {
    if keyword != "while" && keyword != "until" {
        return false;
    }
    let Some(call) = first_child.as_call_node() else { return false };
    let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return false };
    block.closing_loc().span().len() == 3
}

/// RuboCop's `modifier_op?`.
fn is_modifier_op(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::IfNode => node
            .as_if_node()
            .is_some_and(|n| n.if_keyword_loc().is_some() && n.end_keyword_loc().is_none()),
        NodeKind::UnlessNode => {
            node.as_unless_node().is_some_and(|n| n.end_keyword_loc().is_none())
        }
        NodeKind::WhileNode => node
            .as_while_node()
            .is_some_and(|n| !n.is_begin_modifier() && n.closing_loc().is_none()),
        NodeKind::UntilNode => node
            .as_until_node()
            .is_some_and(|n| !n.is_begin_modifier() && n.closing_loc().is_none()),
        NodeKind::RescueModifierNode => true,
        _ => false,
    }
}

/// RuboCop's `SafeAssignment#safe_assignment?`: a parenthesized condition
/// whose single child is an assignment (`lvasgn`/`ivasgn`/`cvasgn`/
/// `gvasgn`/`casgn`/`masgn`, i.e. not an `op_asgn`/`or_asgn`/`and_asgn`
/// shorthand) or a setter-method call (`foo.bar = baz`, recognised in
/// Prism by `CallNode::equal_loc`).
fn is_safe_assignment(paren: &ParenthesesNode<'_>) -> bool {
    let children: Vec<Node<'_>> = paren
        .body()
        .and_then(|body| body.as_statements_node())
        .map(|stmts| stmts.body().iter().collect())
        .unwrap_or_default();
    let [child] = children.as_slice() else { return false };
    is_equals_asgn(child) || child.as_call_node().is_some_and(|c| c.equal_loc().is_some())
}

fn is_equals_asgn(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::LocalVariableWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::MultiWriteNode
    )
}

/// RuboCop's `ParenthesesCorrector.correct` for the plain case (no comment
/// above the closing paren, no chained call after it, no orphaned comma):
/// removes `(` together with its trailing space/tab, then any run of bare
/// newlines, then any further run of Ruby whitespace; removes `)` together
/// with only a leading run of bare newlines.
fn remove_parens(ctx: &Context<'_>, paren: &ParenthesesNode<'_>) -> Vec<Edit> {
    let open = paren.opening_loc().span();
    let close = paren.closing_loc().span();
    let open_range = ctx.with_surrounding_space(open, Side::Right, true, true);
    let close_range = ctx.with_surrounding_space(close, Side::Left, true, false);
    let close_start = close_range.start.max(open_range.end);
    vec![
        Edit::delete(Span::new(open.start, open_range.end)),
        Edit::delete(Span::new(close_start, close.end)),
    ]
}
