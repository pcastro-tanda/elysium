//! `Style/NegatedIf`, ported from RuboCop's
//! `lib/rubocop/cop/style/negated_if.rb`, which mixes in the shared
//! `NegativeConditional` module (`lib/rubocop/cop/mixin/negative_conditional.rb`)
//! and `ConditionCorrector.correct_negative_condition`
//! (`lib/rubocop/cop/correctors/condition_corrector.rb`).
//!
//! # Node shapes
//!
//! whitequark's `unless` is the same `:if` node type as `if`, distinguished
//! only by `node.keyword`; upstream's `return if node.unless?` guard against
//! it is therefore dead here, since Prism gives `unless` its own
//! [`ruby_ast::node::UnlessNode`] kind, which this rule never subscribes to
//! (`Style/NegatedUnless` covers that case instead).
//!
//! `elsif` is another [`ruby_ast::node::IfNode`], reached through the
//! parent's `subsequent()`, distinguished from a genuine `if` only by
//! `if_keyword_loc` reading `"elsif"` rather than `"if"` (see
//! `Lint/DuplicateElsifCondition`'s module doc for the same shape);
//! `node.elsif?` is reproduced with that keyword-text check. A ternary is an
//! `IfNode` with no `if_keyword_loc` at all (`node.ternary?`).
//!
//! `node.if_type? && node.else?` (skipped only for `if`, never `while`/
//! `until`, hence its absence from `NegatedWhile`'s port) covers both a
//! genuine `else` and an `elsif` continuation: both show up as this node's
//! own `subsequent()` being `Some`.
//!
//! `node.modifier_form?` is `end_keyword_loc().is_none()` -- true only once
//! `elsif`/ternary are already excluded, matching rubocop-ast's
//! `(if? || unless?) && super`.
//!
//! # Autocorrection
//!
//! `ConditionCorrector.correct_negative_condition` replaces `node.loc.keyword`
//! (the `if` keyword) with `unless`, and the negated condition (after
//! unwrapping any wrapping `begin`/`ParenthesesNode`s) with its receiver's own
//! source -- identical to `NegatedWhile`'s fix, just keyed on the `if`
//! keyword instead of `while`/`until`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`, specialized: `NegatedIf` only ever inverts `if` into
/// `unless` (never the reverse), unlike the shared `NegativeConditional`
/// module's generic `%<inverse>s`/`%<current>s` interpolation.
const MSG: &str = "Favor `unless` over `if` for negative conditions.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Both,
    Prefix,
    Postfix,
}

/// Favor unless over if for negative conditions (or control flow or).
#[derive(Debug, Clone)]
pub struct NegatedIf {
    style: Style,
}

impl Rule for NegatedIf {
    const META: RuleMeta = RuleMeta {
        name: "Style/NegatedIf",
        department: Department::Style,
        summary: "Favor unless over if for negative conditions (or control flow or).",
        explanation: "Checks for uses of if with a negated condition. Only ifs\nwithout else are considered. There are three different styles:\n\n* both\n* prefix\n* postfix\n\n```ruby\n# EnforcedStyle: both (default)\n# enforces `unless` for `prefix` and `postfix` conditionals\n\n# bad\n\nif !foo\n  bar\nend\n\n# good\n\nunless foo\n  bar\nend\n\n# bad\n\nbar if !foo\n\n# good\n\nbar unless foo\n\n# EnforcedStyle: prefix\n# enforces `unless` for just `prefix` conditionals\n\n# bad\n\nif !foo\n  bar\nend\n\n# good\n\nunless foo\n  bar\nend\n\n# good\n\nbar if !foo\n\n# EnforcedStyle: postfix\n# enforces `unless` for just `postfix` conditionals\n\n# bad\n\nbar if !foo\n\n# good\n\nbar unless foo\n\n# good\n\nif !foo\n  bar\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("both"),
            allowed: &["both", "prefix", "postfix"],
            doc: "`both`: prefix and postfix negated `if` should both use `unless`. \
                  `prefix`: only use `unless` for negated `if` statements positioned before \
                  the body of the statement. `postfix`: only use `unless` for negated `if` \
                  statements positioned after the body of the statement.",
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
        let if_node = node.as_if_node().expect("kind matched");
        // Ternary (no `if`/`elsif` keyword at all).
        let Some(keyword_loc) = if_node.if_keyword_loc() else { return };
        // `elsif` (its own `if`/`elsif` head handles the chain independently).
        if keyword_loc.as_slice() == b"elsif" {
            return;
        }
        // `node.if_type? && node.else?`: a genuine `else`, or a further
        // `elsif` continuation, is `subsequent()` being present.
        if if_node.subsequent().is_some() {
            return;
        }
        let is_modifier_form = if_node.end_keyword_loc().is_none();
        let skip = match self.style {
            Style::Prefix => is_modifier_form,
            Style::Postfix => !is_modifier_form,
            Style::Both => false,
        };
        if skip {
            return;
        }

        let Some(condition) = unwrap_negated_condition(if_node.predicate()) else { return };
        let Some(call) = single_negative(&condition) else { return };

        let receiver = call.receiver().expect("unary `!`/`not` always has a receiver");
        let receiver_text = ctx.text(receiver.span()).to_vec();
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::replace(keyword_loc.span(), b"unless".to_vec()),
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
