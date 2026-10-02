//! `Style/EmptyStringInsideInterpolation`, ported from RuboCop's
//! `lib/rubocop/cop/style/empty_string_inside_interpolation.rb`.
//!
//! Upstream's `Interpolation` mixin fires `on_interpolation` for every
//! `begin` child of a `dstr`/`xstr`/`dsym`/`regexp` node -- i.e. every
//! `#{...}` segment, regardless of which kind of string-like literal it
//! sits inside. Prism represents that segment uniformly as a single
//! [`NodeKind::EmbeddedStatementsNode`] no matter the enclosing literal, so
//! subscribing to just that one kind covers all four upstream hooks.
//!
//! Whitequark's `IfNode#if_branch`/`#else_branch` are "normalized for
//! `unless` nodes" through a double swap (`node_parts`' label-swap undoing
//! the builder's own true/false argument swap for `unless`) that, worked
//! through by hand, ends up being a no-op: `if_branch` is always the
//! primary (then-slot) body and `else_branch` is always the else-clause
//! body, for `if`, `unless`, and ternaries alike. That is exactly how
//! Prism already exposes `statements()`/`subsequent()`-or-`else_clause()`,
//! so [`branches`] reads them directly with no if/unless-specific swap.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::StatementsNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_TRAILING_CONDITIONAL: &str = "Do not use trailing conditionals in string interpolation.";
const MSG_TERNARY: &str = "Do not return empty strings in string interpolation.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    TrailingConditional,
    Ternary,
}

/// Checks for empty strings being assigned inside string interpolation.
#[derive(Debug, Clone)]
pub struct EmptyStringInsideInterpolation {
    style: Style,
}

impl Rule for EmptyStringInsideInterpolation {
    const META: RuleMeta = RuleMeta {
        name: "Style/EmptyStringInsideInterpolation",
        department: Department::Style,
        summary: "Checks for empty strings being assigned inside string interpolation.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::EmbeddedStatementsNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("trailing_conditional"),
            allowed: &["trailing_conditional", "ternary"],
            doc: "Whether an empty-string branch should become a trailing modifier \
                  conditional or stay a ternary.",
        }],
        blind_spots: "If a block-form `if`/`unless` has an empty primary or else branch but \
                      the other side is entirely absent (no `else` at all, matching upstream's \
                      own unguarded `child_node.else_branch.source`/`child_node.if_branch.source` \
                      calls), upstream would raise `NoMethodError` on `nil`; this port instead \
                      silently skips the correction rather than panicking.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "ternary" => Style::Ternary,
            _ => Style::TrailingConditional,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let embedded = node.as_embedded_statements_node().expect("kind matched");
        let Some(stmts) = embedded.statements() else { return };
        for stmt in &stmts.body() {
            if !matches!(stmt.kind(), NodeKind::IfNode | NodeKind::UnlessNode) {
                continue;
            }
            match self.style {
                Style::TrailingConditional => trailing_conditional_correction(&stmt, ctx),
                Style::Ternary => ternary_correction(node, &stmt, ctx),
            }
        }
    }
}

/// The parts of an `if`/`unless`/ternary node this cop cares about, already
/// normalized the way upstream's `if_branch`/`else_branch`/`modifier_form?`
/// are (see the module docs).
struct Branches<'pr> {
    condition: Node<'pr>,
    if_branch: Option<Node<'pr>>,
    else_branch: Option<Node<'pr>>,
    modifier_form: bool,
    is_unless: bool,
}

fn branches<'pr>(node: &Node<'pr>) -> Option<Branches<'pr>> {
    match node.kind() {
        NodeKind::IfNode => {
            let n = node.as_if_node()?;
            let is_ternary = n.if_keyword_loc().is_none();
            let modifier_form = !is_ternary && n.end_keyword_loc().is_none();
            let if_branch = single_stmt(n.statements());
            let else_branch = n
                .subsequent()
                .and_then(|s| s.as_else_node())
                .and_then(|e| single_stmt(e.statements()));
            Some(Branches {
                condition: n.predicate(),
                if_branch,
                else_branch,
                modifier_form,
                is_unless: false,
            })
        }
        NodeKind::UnlessNode => {
            let n = node.as_unless_node()?;
            let modifier_form = n.end_keyword_loc().is_none();
            let if_branch = single_stmt(n.statements());
            let else_branch = n.else_clause().and_then(|e| single_stmt(e.statements()));
            Some(Branches {
                condition: n.predicate(),
                if_branch,
                else_branch,
                modifier_form,
                is_unless: true,
            })
        }
        _ => None,
    }
}

/// A branch body is a single statement (whitequark elides a single-child
/// `begin`, so a `(str)`/`(nil)`/anything-else node, never a multi-statement
/// `begin`, is what `if_branch`/`else_branch` observe).
fn single_stmt(stmts: Option<StatementsNode<'_>>) -> Option<Node<'_>> {
    let body = stmts?.body();
    (body.len() == 1).then(|| body.first()).flatten()
}

/// RuboCop's `empty_branch_outcome?`: `nil` literal, or an empty (non-
/// interpolated) string literal.
fn is_empty_branch(node: &Node<'_>) -> bool {
    node.kind() == NodeKind::NilNode
        || node.as_string_node().is_some_and(|s| s.unescaped().is_empty())
}

fn trailing_conditional_correction(child: &Node<'_>, ctx: &mut Context<'_>) {
    let Some(b) = branches(child) else { return };
    if b.modifier_form {
        return;
    }
    if let (Some(if_branch), Some(else_branch)) = (b.if_branch, b.else_branch) {
        if is_empty_branch(&if_branch) {
            ternary_style_autocorrect(
                child.span(),
                else_branch.span(),
                "unless",
                b.condition.span(),
                ctx,
            );
        }
    }
    let (Some(if_branch), Some(else_branch)) = (b.if_branch, b.else_branch) else { return };
    if !is_empty_branch(&else_branch) {
        return;
    }
    ternary_style_autocorrect(child.span(), if_branch.span(), "if", b.condition.span(), ctx);
}

fn ternary_style_autocorrect(
    child_span: Span,
    outcome_span: Span,
    keyword: &str,
    condition_span: Span,
    ctx: &mut Context<'_>,
) {
    let outcome = String::from_utf8_lossy(ctx.text(outcome_span));
    let condition = String::from_utf8_lossy(ctx.text(condition_span));
    let replacement = format!("{outcome} {keyword} {condition}");
    ctx.report_with_fix(
        &EmptyStringInsideInterpolation::META,
        child_span,
        MSG_TERNARY,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(child_span, replacement.into_bytes())],
        },
    );
}

fn ternary_correction(embedded_node: &Node<'_>, child: &Node<'_>, ctx: &mut Context<'_>) {
    let Some(b) = branches(child) else { return };
    if !b.modifier_form {
        return;
    }
    let Some(if_branch) = b.if_branch else { return };
    let if_source = String::from_utf8_lossy(ctx.text(if_branch.span())).into_owned();
    let ternary_component =
        if b.is_unless { format!("'' : {if_source}") } else { format!("{if_source} : ''") };

    let condition = String::from_utf8_lossy(ctx.text(b.condition.span()));
    let replacement = format!("#{{{condition} ? {ternary_component}}}");

    ctx.report_with_fix(
        &EmptyStringInsideInterpolation::META,
        embedded_node.span(),
        MSG_TRAILING_CONDITIONAL,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(embedded_node.span(), replacement.into_bytes())],
        },
    );
}
