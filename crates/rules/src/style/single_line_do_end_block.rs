//! `Style/SingleLineDoEndBlock`, ported from RuboCop's
//! `lib/rubocop/cop/style/single_line_do_end_block.rb`, with
//! `lib/rubocop/cop/mixin/check_single_line_suitability.rb` inlined.
//!
//! A Prism `LambdaNode` (`->(x) do ... end`) is structurally distinct from
//! `CallNode` + `BlockNode`, unlike whitequark's modernized AST where it is
//! still a block node whose send is a `LambdaNode`/`lambda_literal?`. Both
//! Prism kinds share the same opening/closing/parameters/body shape, so
//! they are handled uniformly here; the upstream `node.send_node
//! .lambda_literal?` check (which forces inserting the line break right
//! after `do` instead of after the block parameters) becomes "is this a
//! `LambdaNode`". Also unlike whitequark (where `on_block`'s node already
//! spans from the call's receiver/selector through the block's `end`), a
//! Prism `BlockNode`'s own span starts at its opening (`do`/`{`); the full
//! offense/report range is the owning `CallNode`'s span instead.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::is_heredoc;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Prefer multiline `do`...`end` block.";

/// Checks for single-line `do`...`end` blocks.
#[derive(Debug, Clone)]
pub struct SingleLineDoEndBlock {
    inspect_blocks: bool,
    max_line_length: Option<i64>,
}

impl Rule for SingleLineDoEndBlock {
    const META: RuleMeta = RuleMeta {
        name: "Style/SingleLineDoEndBlock",
        department: Department::Style,
        summary: "Checks for single-line `do`...`end` blocks.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::BlockNode, NodeKind::LambdaNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let inspect_blocks = options
            .peer("Layout/RedundantLineBreak", "InspectBlocks")
            .and_then(OptionValue::as_bool)
            .unwrap_or(false);
        let line_length_enabled = options
            .peer("Layout/LineLength", "Enabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(true);
        let max_line_length = line_length_enabled.then(|| {
            options.peer("Layout/LineLength", "Max").and_then(OptionValue::as_int).unwrap_or(120)
        });
        Ok(Self { inspect_blocks, max_line_length })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (opening, closing, parameters, body, is_lambda) = match node {
            Node::BlockNode { .. } => {
                let b = node.as_block_node().expect("kind checked by dispatch");
                (b.opening_loc(), b.closing_loc(), b.parameters(), b.body(), false)
            }
            Node::LambdaNode { .. } => {
                let l = node.as_lambda_node().expect("kind checked by dispatch");
                (l.opening_loc(), l.closing_loc(), l.parameters(), l.body(), true)
            }
            _ => return,
        };

        // `node.braces?`: brace-delimited blocks are naturally single-line.
        if opening.as_slice() == b"{" {
            return;
        }
        let node_span = node.span();
        if !ctx.is_single_line(node_span) {
            return;
        }
        let report_span = match ctx.parent() {
            Some(parent) if parent.kind == NodeKind::CallNode => parent.span,
            _ => node_span,
        };
        if self.inspect_blocks
            && suitable_as_single_line(ctx, node_span, body.as_ref(), self.max_line_length)
        {
            return;
        }

        let do_line_end = if is_lambda || uses_opening_for_do_line(parameters.as_ref()) {
            opening.span().end
        } else {
            parameters.expect("non-opening branch implies parameters present").span().end
        };

        let mut edits = vec![Edit::insert(do_line_end, b"\n".to_vec())];
        match body.and_then(|b| trailing_heredoc(ctx, &b)) {
            Some(heredoc_end) => {
                edits.push(Edit::delete(closing.span()));
                edits.push(Edit::insert(heredoc_end, b"\nend".to_vec()));
            }
            None => edits.push(Edit::insert(closing.span().start, b"\n".to_vec())),
        }

        ctx.report_with_fix(
            &Self::META,
            report_span,
            MSG,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// `node.arguments.children.empty?`: no block parameters at all (bare `do
/// ... end`), a numbered (`_1`) or `it` parameter list, or an explicit but
/// empty `||` parameter list.
fn uses_opening_for_do_line(parameters: Option<&Node<'_>>) -> bool {
    match parameters {
        None => true,
        Some(p) => match p.kind() {
            NodeKind::NumberedParametersNode | NodeKind::ItParametersNode => true,
            NodeKind::BlockParametersNode => {
                p.as_block_parameters_node().is_some_and(|bp| bp.parameters().is_none())
            }
            _ => false,
        },
    }
}

/// The heredoc opened within `body` (the body itself or any descendant)
/// whose body extends the furthest down, or `None`. Returns the offset to
/// insert `"\nend"` after -- the end of its terminator line, excluding the
/// line's own trailing newline.
fn trailing_heredoc(ctx: &Context<'_>, body: &Node<'_>) -> Option<u32> {
    let mut candidates = vec![*body];
    ruby_ast::each_descendant(body, &mut |child| candidates.push(*child));

    let mut best: Option<(u32, u32)> = None; // (line, end offset)
    for candidate in candidates {
        if !is_heredoc(&candidate) {
            continue;
        }
        let Some(closing) = string_closing_loc(&candidate) else { continue };
        let span = closing.span();
        let end = if closing.as_slice().ends_with(b"\n") { span.end - 1 } else { span.end };
        let line = ctx.line_col(span.start).line;
        if best.is_none_or(|(best_line, _)| line > best_line) {
            best = Some((line, end));
        }
    }
    best.map(|(_, end)| end)
}

/// The closing (terminator) location of a heredoc-capable string literal.
fn string_closing_loc<'pr>(node: &Node<'pr>) -> Option<ruby_ast::Location<'pr>> {
    match node {
        Node::StringNode { .. } => node.as_string_node()?.closing_loc(),
        Node::InterpolatedStringNode { .. } => node.as_interpolated_string_node()?.closing_loc(),
        Node::XStringNode { .. } => Some(node.as_x_string_node()?.closing_loc()),
        Node::InterpolatedXStringNode { .. } => {
            Some(node.as_interpolated_x_string_node()?.closing_loc())
        }
        _ => None,
    }
}

/// `CheckSingleLineSuitability#suitable_as_single_line?`.
fn suitable_as_single_line(
    ctx: &Context<'_>,
    span: Span,
    body: Option<&Node<'_>>,
    max_line_length: Option<i64>,
) -> bool {
    !too_long(ctx, span, max_line_length) && !comment_within(ctx, span) && safe_to_split(body)
}

fn too_long(ctx: &Context<'_>, span: Span, max_line_length: Option<i64>) -> bool {
    let Some(max) = max_line_length else { return false };
    let line = ctx.line_col(span.start).line;
    let length = String::from_utf8_lossy(ctx.line_text(line)).chars().count();
    i64::try_from(length).unwrap_or(i64::MAX) > max
}

fn comment_within(ctx: &Context<'_>, span: Span) -> bool {
    let first_line = ctx.line_col(span.start).line;
    let last_line = ctx.line_col(span.end.saturating_sub(1).max(span.start)).line;
    ctx.comments().iter().any(|comment| (first_line..=last_line).contains(&comment.line))
}

/// `CheckSingleLineSuitability#safe_to_split?`: no `if`/`case`/standalone
/// `begin`/`def`/`rescue`/`ensure` descendant, and no heredoc or
/// embedded-newline string/symbol descendant.
fn safe_to_split(body: Option<&Node<'_>>) -> bool {
    let Some(body) = body else { return true };
    let mut safe = true;
    ruby_ast::each_descendant(body, &mut |node| {
        if !safe || !node_is_safe(node) {
            safe = false;
        }
    });
    safe
}

fn node_is_safe(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::IfNode
        | NodeKind::UnlessNode
        | NodeKind::CaseNode
        | NodeKind::CaseMatchNode
        | NodeKind::DefNode
        | NodeKind::RescueNode
        | NodeKind::EnsureNode => false,
        NodeKind::BeginNode => node.as_begin_node().is_none_or(|b| b.begin_keyword_loc().is_none()),
        NodeKind::ParenthesesNode => !node
            .as_parentheses_node()
            .is_some_and(|p| p.is_multiple_statements() && !is_node_single_line(node)),
        NodeKind::SymbolNode => is_node_single_line(node),
        _ if is_heredoc(node) => false,
        _ => !string_contains_newline(node),
    }
}

/// Whether `node`'s own source text (which, for a heredoc-opening string,
/// excludes the deferred body) spans a single line.
fn is_node_single_line(node: &Node<'_>) -> bool {
    !node.location().as_slice().contains(&b'\n')
}

fn string_contains_newline(node: &Node<'_>) -> bool {
    match node {
        Node::StringNode { .. } => {
            node.as_string_node().is_some_and(|s| s.unescaped().contains(&b'\n'))
        }
        Node::InterpolatedStringNode { .. } => !is_node_single_line(node),
        _ => false,
    }
}
