//! `Style/InfiniteLoop`, ported from RuboCop's
//! `lib/rubocop/cop/style/infinite_loop.rb`, which mixes in the shared
//! `Alignment` module (`lib/rubocop/cop/mixin/alignment.rb`).
//!
//! # The `@variables` guard
//!
//! Upstream joins forces with `VariableForce`, whose own full-file traversal
//! (`Team#investigate_partial` runs every joined force's `#investigate`
//! before the Commissioner's own `walk`, see `lib/rubocop/cop/commissioner.rb`)
//! completes before any `on_while`/`on_until` callback fires. Its
//! `after_leaving_scope` hook appends every scope's variables to `@variables`
//! as each scope closes, so by the time a `while`/`until` node is visited,
//! `@variables` already holds every local variable declared anywhere in the
//! file. [`blocked_by_variable`] reproduces that by reading
//! [`Context::semantics`] (built once, lazily, over the whole file) instead of
//! a running `@variables` accumulator.
//!
//! # `begin...end while`/`until` (post-condition loops)
//!
//! Prism represents this as an ordinary `WhileNode`/`UntilNode` with
//! `is_begin_modifier` set, whose `statements` wraps a single
//! [`ruby_ast::node::BeginNode`] (the `begin`/`end` keywords are present even
//! for a single-statement body, unlike whitequark's `kwbegin`); see
//! `Lint/Loop`'s module doc for the same shape.
//!
//! # Modifier-form autocorrection
//!
//! `modifier_replacement`'s multi-line branch reproduces
//! `Alignment#indentation`/`#offset` and the `LEADING_SPACE` regex literally:
//! the gsub indent is the node's own column plus the configured width, and
//! the outer `join("\n#{indentation}")` separator is a *second*,
//! independent indent taken from the leading whitespace of the body's own
//! first physical line -- upstream applies both, even though in every
//! example in this corpus the second one is empty.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::StatementsNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `Kernel#loop` for infinite loops.";

/// Use Kernel#loop for infinite loops.
#[derive(Debug, Clone)]
pub struct InfiniteLoop {
    /// `Alignment#configured_indentation_width`: `Layout/IndentationWidth`'s
    /// `Width`, else `2`.
    indentation_width: i64,
}

impl Rule for InfiniteLoop {
    const META: RuleMeta = RuleMeta {
        name: "Style/InfiniteLoop",
        department: Department::Style,
        summary: "Use Kernel#loop for infinite loops. This cop is unsafe if the body may raise a `StopIteration` exception.",
        explanation: "Use `Kernel#loop` for infinite loops.\n\n\
                      @safety\n  \
                      This cop is unsafe as the rule should not necessarily apply if the loop\n  \
                      body might raise a `StopIteration` exception; contrary to other infinite\n  \
                      loops, `Kernel#loop` silently rescues that and returns `nil`.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::WhileNode, NodeKind::UntilNode],
        config: &[],
        blind_spots: "\
`Layout/IndentationWidth`'s `Width` is read as a peer option (falling back to \
2), matching upstream's own `Alignment#configured_indentation_width` \
cross-cop read.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .unwrap_or(2);
        Ok(Self { indentation_width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let shape = match node.kind() {
            NodeKind::WhileNode => {
                let w = node.as_while_node().expect("kind matched");
                if !is_truthy_literal(&w.predicate()) {
                    return;
                }
                LoopShape {
                    is_begin_modifier: w.is_begin_modifier(),
                    keyword_span: w.keyword_loc().span(),
                    do_keyword_span: w.do_keyword_loc().map(|l| l.span()),
                    closing_span: w.closing_loc().map(|l| l.span()),
                    predicate_span: w.predicate().span(),
                    statements: w.statements(),
                }
            }
            NodeKind::UntilNode => {
                let u = node.as_until_node().expect("kind matched");
                if !is_falsey_literal(&u.predicate()) {
                    return;
                }
                LoopShape {
                    is_begin_modifier: u.is_begin_modifier(),
                    keyword_span: u.keyword_loc().span(),
                    do_keyword_span: u.do_keyword_loc().map(|l| l.span()),
                    closing_span: u.closing_loc().map(|l| l.span()),
                    predicate_span: u.predicate().span(),
                    statements: u.statements(),
                }
            }
            _ => return,
        };

        let node_span = node.span();
        if blocked_by_variable(node_span, ctx) {
            return;
        }
        let Some(fix) = self.autocorrect(node_span, &shape, ctx) else { return };
        ctx.report_with_fix(&Self::META, shape.keyword_span, MSG, fix);
    }
}

impl InfiniteLoop {
    /// RuboCop's `autocorrect`.
    fn autocorrect(
        &self,
        node_span: Span,
        shape: &LoopShape<'_>,
        ctx: &Context<'_>,
    ) -> Option<Fix> {
        let edits = if shape.is_begin_modifier {
            Self::replace_begin_end_with_modifier(node_span, shape)?
        } else if shape.closing_span.is_none() {
            vec![Edit::replace(node_span, self.modifier_replacement(node_span, shape, ctx)?)]
        } else {
            let end = shape.do_keyword_span.map_or(shape.predicate_span.end, |d| d.end);
            vec![Edit::replace(Span::new(shape.keyword_span.start, end), *b"loop do")]
        };
        Some(Fix { applicability: Applicability::Unsafe, edits })
    }

    /// RuboCop's `replace_begin_end_with_modifier`.
    fn replace_begin_end_with_modifier(
        node_span: Span,
        shape: &LoopShape<'_>,
    ) -> Option<Vec<Edit>> {
        let statements = shape.statements?;
        let begin_node = statements.body().iter().next()?.as_begin_node()?;
        let begin_kw = begin_node.begin_keyword_loc()?.span();
        let end_kw = begin_node.end_keyword_loc()?.span();
        Some(vec![
            Edit::replace(begin_kw, *b"loop do"),
            Edit::delete(Span::new(end_kw.end, node_span.end)),
        ])
    }

    /// RuboCop's `modifier_replacement`.
    fn modifier_replacement(
        &self,
        node_span: Span,
        shape: &LoopShape<'_>,
        ctx: &Context<'_>,
    ) -> Option<Box<[u8]>> {
        let body = shape.statements?.body().iter().next()?;
        let body_span = body.span();
        if ctx.is_single_line(node_span) {
            let mut replacement = b"loop { ".to_vec();
            replacement.extend_from_slice(ctx.text(body_span));
            replacement.extend_from_slice(b" }");
            return Some(replacement.into());
        }

        let node_column = ctx.line_col(node_span.start).column;
        let width = usize::try_from(self.indentation_width.max(0)).unwrap_or(2);
        let gsub_indent_len = node_column as usize + width;
        let gsub_indent = vec![b' '; gsub_indent_len];

        let mut modified_body = gsub_indent.clone();
        for &byte in ctx.text(body_span) {
            modified_body.push(byte);
            if byte == b'\n' {
                modified_body.extend_from_slice(&gsub_indent);
            }
        }

        let body_line = ctx.line_col(body_span.start).line;
        let line_text = ctx.line_text(body_line);
        let join_indent_len = line_text.iter().take_while(|&&b| b == b' ' || b == b'\t').count();
        let join_indent = &line_text[..join_indent_len];

        let mut replacement = b"loop do\n".to_vec();
        replacement.extend_from_slice(join_indent);
        replacement.extend_from_slice(&modified_body);
        replacement.push(b'\n');
        replacement.extend_from_slice(join_indent);
        replacement.extend_from_slice(b"end");
        Some(replacement.into())
    }
}

/// The `while`/`until` fields the two node kinds share, unpacked once so the
/// rest of the cop is kind-agnostic.
struct LoopShape<'pr> {
    /// RuboCop's `post_condition_loop?`: `begin...end while`/`until`.
    is_begin_modifier: bool,
    /// The `while`/`until` keyword's own span, where the offense is reported.
    keyword_span: Span,
    /// The `do` keyword's span, if the block form spells it out.
    do_keyword_span: Option<Span>,
    /// The closing `end`'s span; `None` for both modifier forms.
    closing_span: Option<Span>,
    predicate_span: Span,
    statements: Option<StatementsNode<'pr>>,
}

/// RuboCop's `while_or_until`: skip the offense when a variable is assigned
/// inside the loop, never assigned before it, and read again after it --
/// wrapping the loop in a block would put that variable's first assignment
/// out of scope for the code following the loop.
fn blocked_by_variable(range: Span, ctx: &Context<'_>) -> bool {
    let semantics = ctx.semantics();
    semantics.variables().iter().any(|variable| {
        let mut assigned_inside = false;
        let mut assigned_before = false;
        for &id in variable.assignments() {
            let span = semantics.assignment(id).node().span();
            assigned_inside |= range.contains(span);
            assigned_before |= span.end < range.start;
        }
        assigned_inside
            && !assigned_before
            && variable.references().iter().any(|r| r.node().span().start > range.end)
    })
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
