//! `Style/AmbiguousEndlessMethodDefinition`, ported from RuboCop's
//! `lib/rubocop/cop/style/ambiguous_endless_method_definition.rb` plus the
//! `EndlessMethodRewriter` mixin it includes.
//!
//! # Matched shape
//!
//! Upstream's `^${(if _ <def _>) ({and or} def _) ({while until} _ def)}`
//! node-matcher captures the *parent* of an endless `def` when that parent
//! is an `if`/`unless` whose body is the lone endless `def` (whitequark
//! elides the single-statement body, so the branch *is* the `def` node
//! directly), an `and`/`or` node whose left operand is the `def`, or a
//! `while`/`until` node whose body is the lone `def`. Because the `if`/
//! `unless` branch of the pattern requires the whole node, and a modifier
//! `if`/`unless` can never carry an `elsif`/`else` (the grammar forbids it),
//! checking `modifier_form?` on the captured node after the fact is
//! equivalent to requiring the `def` be the *only* statement in the body
//! with no further branches -- this port checks that directly instead of
//! reproducing the pattern's "any remaining position" shape.
//!
//! Prism always wraps a block's/branch's body in a `StatementsNode`, even
//! for a single statement (unlike whitequark, which elides the wrapper), so
//! [`sole_statement`] unwraps it to find the lone `def`.
//!
//! # `and`/`or` vs `&&`/`||`
//!
//! Whitequark represents both spellings as the same `and`/`or` node type;
//! Prism's [`NodeKind::AndNode`]/[`NodeKind::OrNode`] do too, distinguished
//! only by `operator_loc`'s text (see `style/and_or.rs`'s
//! `process_logical_operator`). `modifier_form?(operation)` short-circuits
//! true via `operator_keyword?` whenever the operator is spelled `and`/`or`
//! (regardless of anything else), and the symbol spellings never reach a
//! `modifier_form?` fallback that would make them true, so `&&`/`||` never
//! offend -- confirmed by the cop's own spec ("does not register an
//! offense for `&&`" / "does not register an offense for `||`").
//!
//! # Autocorrection
//!
//! [`correct_to_multiline`] ports `EndlessMethodRewriter#correct_to_multiline`:
//! replaces the `def` node's own source (stopping right before any trailing
//! `if`/`and`/... text, which is untouched) with a 3-line multiline method,
//! indented to the `def`'s own column plus `Layout/IndentationWidth`'s
//! configured `Width` (falling back to `2`, matching
//! `config.for_cop('Layout/IndentationWidth')['Width'] || 2`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::DefNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Avoid using `%<keyword>s` statements with endless methods.";

/// Looks for endless methods inside operators of lower precedence.
#[derive(Debug, Clone)]
pub struct AmbiguousEndlessMethodDefinition {
    /// `Layout/IndentationWidth`'s configured `Width`, falling back to `2`.
    indentation_width: u32,
}

impl Rule for AmbiguousEndlessMethodDefinition {
    const META: RuleMeta = RuleMeta {
        name: "Style/AmbiguousEndlessMethodDefinition",
        department: Department::Style,
        summary: "Checks for endless methods inside operators of lower precedence.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::IfNode,
            NodeKind::UnlessNode,
            NodeKind::AndNode,
            NodeKind::OrNode,
            NodeKind::WhileNode,
            NodeKind::UntilNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .and_then(|w| u32::try_from(w).ok())
            .unwrap_or(2);
        Ok(Self { indentation_width: width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::IfNode { .. } => {
                let Some(if_node) = node.as_if_node() else { return };
                if if_node.if_keyword_loc().is_none() || if_node.end_keyword_loc().is_some() {
                    return; // ternary, or a regular (non-modifier) `if`.
                }
                let Some(def) = sole_statement(if_node.statements()) else { return };
                self.check(&def, node.span(), b"if", ctx);
            }
            Node::UnlessNode { .. } => {
                let Some(unless_node) = node.as_unless_node() else { return };
                if unless_node.end_keyword_loc().is_some() {
                    return;
                }
                let Some(def) = sole_statement(unless_node.statements()) else { return };
                self.check(&def, node.span(), b"unless", ctx);
            }
            Node::AndNode { .. } => {
                let Some(and) = node.as_and_node() else { return };
                let Some(def) = as_endless_def(&and.left()) else { return };
                let keyword = ctx.text(and.operator_loc().span());
                if keyword != b"and" {
                    return; // `&&` never offends: see the module doc.
                }
                self.check(&def, node.span(), keyword, ctx);
            }
            Node::OrNode { .. } => {
                let Some(or) = node.as_or_node() else { return };
                let Some(def) = as_endless_def(&or.left()) else { return };
                let keyword = ctx.text(or.operator_loc().span());
                if keyword != b"or" {
                    return; // `||` never offends.
                }
                self.check(&def, node.span(), keyword, ctx);
            }
            Node::WhileNode { .. } => {
                let Some(w) = node.as_while_node() else { return };
                if w.closing_loc().is_some() {
                    return; // not written in modifier form.
                }
                let Some(def) = sole_statement(w.statements()) else { return };
                self.check(&def, node.span(), b"while", ctx);
            }
            Node::UntilNode { .. } => {
                let Some(u) = node.as_until_node() else { return };
                if u.closing_loc().is_some() {
                    return;
                }
                let Some(def) = sole_statement(u.statements()) else { return };
                self.check(&def, node.span(), b"until", ctx);
            }
            _ => {}
        }
    }
}

impl AmbiguousEndlessMethodDefinition {
    fn check(
        &self,
        def: &DefNode<'_>,
        operation_span: Span,
        keyword: &[u8],
        ctx: &mut Context<'_>,
    ) {
        let keyword = String::from_utf8_lossy(keyword);
        let message = MSG.replace("%<keyword>s", &keyword);
        let edits = vec![Edit::replace(
            def.as_node().span(),
            correct_to_multiline(def, ctx, self.indentation_width),
        )];
        ctx.report_with_fix(
            &Self::META,
            operation_span,
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// The lone statement of `statements`, unwrapping Prism's always-present
/// `StatementsNode` -- `None` for an empty/absent body or one with several
/// statements (whitequark's elided single-statement body is the only shape
/// the upstream pattern matches).
fn sole_statement(statements: Option<ruby_ast::node::StatementsNode<'_>>) -> Option<DefNode<'_>> {
    let body = statements?.body();
    if body.len() != 1 {
        return None;
    }
    as_endless_def(&body.iter().next()?)
}

/// `node` itself, if it is an endless `def` (`equal_loc` present -- the `=`
/// before the body).
fn as_endless_def<'pr>(node: &Node<'pr>) -> Option<DefNode<'pr>> {
    let def = node.as_def_node()?;
    def.equal_loc().is_some().then_some(def)
}

/// RuboCop's `EndlessMethodRewriter#correct_to_multiline`.
fn correct_to_multiline(def: &DefNode<'_>, ctx: &Context<'_>, width: u32) -> Vec<u8> {
    let column = ctx.line_col(def.as_node().span().start).column;
    let indent_body = " ".repeat((column + width) as usize);
    let indent_end = " ".repeat(column as usize);

    let name = String::from_utf8_lossy(def.name().as_slice()).into_owned();
    let receiver = def.receiver().map_or(Vec::new(), |recv| {
        let mut text = ctx.text(recv.span()).to_vec();
        if let Some(op) = def.operator_loc() {
            text.extend_from_slice(ctx.text(op.span()));
        }
        text
    });
    let arguments = arguments_text(def, ctx);
    let body_source = def.body().map_or(Vec::new(), |b| ctx.text(b.span()).to_vec());

    let mut out = Vec::new();
    out.extend_from_slice(b"def ");
    out.extend_from_slice(&receiver);
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(&arguments);
    out.push(b'\n');
    out.extend_from_slice(indent_body.as_bytes());
    out.extend_from_slice(&body_source);
    out.push(b'\n');
    out.extend_from_slice(indent_end.as_bytes());
    out.extend_from_slice(b"end");
    out
}

/// RuboCop's `arguments(node)`: the parameter list's own source, including
/// its parentheses when written with any, or empty when the method takes
/// none.
fn arguments_text(def: &DefNode<'_>, ctx: &Context<'_>) -> Vec<u8> {
    let Some(params) = def.parameters() else { return Vec::new() };
    let has_any = !params.requireds().is_empty()
        || !params.optionals().is_empty()
        || params.rest().is_some()
        || !params.posts().is_empty()
        || !params.keywords().is_empty()
        || params.keyword_rest().is_some()
        || params.block().is_some();
    if !has_any {
        return Vec::new();
    }
    let span = match (def.lparen_loc(), def.rparen_loc()) {
        (Some(l), Some(r)) => Span::new(l.span().start, r.span().end),
        _ => params.as_node().span(),
    };
    ctx.text(span).to_vec()
}
