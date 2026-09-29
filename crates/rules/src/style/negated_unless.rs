//! `Style/NegatedUnless`, ported from RuboCop's
//! `lib/rubocop/cop/style/negated_unless.rb`, which mixes in the shared
//! `NegativeConditional` module (`lib/rubocop/cop/mixin/negative_conditional.rb`).
//!
//! Whitequark folds `unless` into the same `:if` node type as `if`, so
//! upstream's single `on_if` handler (guarded by `node.if? || node.elsif? ||
//! node.ternary?`) only ever actually fires its body for genuine `unless`
//! nodes. Prism keeps `unless` as its own distinct [`ruby_ast::node::UnlessNode`]
//! kind (no ternary/elsif form exists for it), so this port subscribes to
//! that kind directly and drops the whitequark-only guard.
//!
//! whitequark's `node.if_type? && node.else?` guard in
//! `NegativeConditional#check_negative_conditional` is always true for an
//! `unless` (its AST node type is still `:if`), so an `unless`/`else` is
//! always skipped; here that's `node.else_clause().is_some()`.
//!
//! `node.modifier_form?` (`loc.end.nil?` in whitequark) becomes
//! `end_keyword_loc().is_none()` -- `unless` has no ternary form, so a
//! missing `end` keyword unambiguously means the modifier (postfix) form.
//!
//! Whitequark's `begin` node (any parenthesized/grouped condition) is
//! Prism's `ParenthesesNode` wrapping a `StatementsNode`; `condition =
//! condition.children.last while condition.begin_type?` becomes a loop that
//! unwraps nested `ParenthesesNode`s down to their last statement, and an
//! empty `()` (`ParenthesesNode` with no `body`) reproduces
//! `empty_condition?`'s `(begin)` match by returning `None` on the way in.
//! `!x`/`not x` both parse as a `CallNode` named `!` with `x` as its
//! `receiver`; `single_negative?`'s exclusion of `(send _ :!)` receivers
//! rules out the doubly-negated `!!x` case.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// `RuboCop::Cop::Style::NegatedUnless`'s `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Both,
    Prefix,
    Postfix,
}

/// Favor if over unless for negative conditions.
#[derive(Debug, Clone)]
pub struct NegatedUnless {
    style: Style,
}

impl Rule for NegatedUnless {
    const META: RuleMeta = RuleMeta {
        name: "Style/NegatedUnless",
        department: Department::Style,
        summary: "Favor if over unless for negative conditions.",
        explanation: "Checks for uses of `unless` with a negated condition. Only `unless` \
                      without `else` are considered. There are three different styles:\n\n\
                      * `both`\n* `prefix`\n* `postfix`\n\n\
                      ```ruby\n\
                      # EnforcedStyle: both (default)\n\
                      # enforces `if` for `prefix` and `postfix` conditionals\n\n\
                      # bad\n\
                      unless !foo\n  bar\nend\n\n\
                      # good\n\
                      if foo\n  bar\nend\n\n\
                      # bad\n\
                      bar unless !foo\n\n\
                      # good\n\
                      bar if foo\n\n\
                      # EnforcedStyle: prefix\n\
                      # enforces `if` for just `prefix` conditionals\n\n\
                      # bad\n\
                      unless !foo\n  bar\nend\n\n\
                      # good\n\
                      if foo\n  bar\nend\n\n\
                      # good\n\
                      bar unless !foo\n\n\
                      # EnforcedStyle: postfix\n\
                      # enforces `if` for just `postfix` conditionals\n\n\
                      # bad\n\
                      bar unless !foo\n\n\
                      # good\n\
                      bar if foo\n\n\
                      # good\n\
                      unless !foo\n  bar\nend\n\
                      ```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::UnlessNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("both"),
            allowed: &["both", "prefix", "postfix"],
            doc: "Whether to flag negated conditions in both prefix (`unless ... end`) and \
                  postfix (`... unless ...`) `unless`, or just one of the two forms.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "prefix" => Style::Prefix,
            "postfix" => Style::Postfix,
            _ => Style::Both,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(unless_node) = node.as_unless_node() else { return };

        // RuboCop's `NegativeConditional#check_negative_conditional`'s
        // `node.if_type? && node.else?` guard: always true for `unless`.
        if unless_node.else_clause().is_some() {
            return;
        }

        // RuboCop's `correct_style?`: `loc.end.nil?` (whitequark) is
        // `end_keyword_loc().is_none()` here -- `unless` has no ternary
        // form, so this is unambiguously the modifier (postfix) form.
        let modifier_form = unless_node.end_keyword_loc().is_none();
        let skip = match self.style {
            Style::Both => false,
            Style::Prefix => modifier_form,
            Style::Postfix => !modifier_form,
        };
        if skip {
            return;
        }

        let Some(condition) = unwrap_negated_condition(unless_node.predicate()) else { return };
        let Some(call) = single_negative(&condition) else { return };

        let message = "Favor `if` over `unless` for negative conditions.";
        // RuboCop's `ConditionCorrector.correct_negative_condition`: replace
        // the `unless` keyword with `if`, and the negated condition with its
        // receiver's own source (`condition.children.first.source`).
        let receiver = call.receiver().expect("unary `!`/`not` always has a receiver");
        let receiver_text = ctx.text(receiver.span()).to_vec();
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::replace(unless_node.keyword_loc().span(), b"if".to_vec()),
                    Edit::replace(condition.span(), receiver_text),
                ],
            },
        );
    }
}

/// RuboCop's `condition = condition.children.last while condition.begin_type?`,
/// preceded by `return if empty_condition?(condition)`: an empty `()` at any
/// unwrapping step (a `ParenthesesNode` with no `body`) is `(begin)`'s empty
/// match, so it short-circuits to `None` here exactly as the early return
/// upstream does.
fn unwrap_negated_condition(mut node: Node<'_>) -> Option<Node<'_>> {
    while let Some(paren) = node.as_parentheses_node() {
        let body = paren.body()?;
        let stmts = body.as_statements_node()?;
        node = stmts.body().last()?;
    }
    Some(node)
}

/// RuboCop's `single_negative?`: `(send !(send _ :!) :!)` -- a `!`/`not`
/// call whose receiver is not itself a `!`/`not` call (which would make the
/// condition doubly negated, e.g. `!!foo`).
fn single_negative<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"!" {
        return None;
    }
    if call
        .receiver()
        .is_some_and(|r| r.as_call_node().is_some_and(|c| c.name().as_slice() == b"!"))
    {
        return None;
    }
    Some(call)
}
