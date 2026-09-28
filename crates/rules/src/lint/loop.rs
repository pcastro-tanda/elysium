//! `Lint/Loop`, ported from RuboCop's
//! `lib/rubocop/cop/lint/loop.rb`.
//!
//! `on_while_post`/`on_until_post` only ever fire for whitequark's
//! `begin...end while/until` shape; Prism represents the same construct as a
//! `WhileNode`/`UntilNode` with `is_begin_modifier` set, whose single
//! statement is the wrapped `BeginNode` (`begin`/`end` keywords included even
//! for a single-statement body, unlike whitequark's `kwbegin`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::StatementsNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `Kernel#loop` with `break` rather than `begin/end/until`(or `while`).";

/// Use Kernel#loop with break rather than begin/end/until or begin/end/while for post-loop tests.
#[derive(Debug, Clone)]
pub struct Loop;

impl Rule for Loop {
    const META: RuleMeta = RuleMeta {
        name: "Lint/Loop",
        department: Department::Lint,
        summary: "Use Kernel#loop with break rather than begin/end/until or begin/end/while for post-loop tests.",
        explanation: "Checks for uses of `begin...end while/until something`. `Kernel#loop` \
                      with `break` should be preferred, since the behaviour of `begin/end` \
                      post-condition loops (running the body at least once, even when the \
                      condition is already false) is a frequent source of confusion.",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::WhileNode, NodeKind::UntilNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::WhileNode => {
                let w = node.as_while_node().expect("kind matched");
                if w.is_begin_modifier() {
                    register_offense(
                        ctx,
                        node.span(),
                        w.keyword_loc().span(),
                        w.predicate().span(),
                        w.statements(),
                        "unless",
                    );
                }
            }
            NodeKind::UntilNode => {
                let u = node.as_until_node().expect("kind matched");
                if u.is_begin_modifier() {
                    register_offense(
                        ctx,
                        node.span(),
                        u.keyword_loc().span(),
                        u.predicate().span(),
                        u.statements(),
                        "if",
                    );
                }
            }
            _ => {}
        }
    }
}

/// RuboCop's `register_offense`.
fn register_offense(
    ctx: &mut Context<'_>,
    node_span: Span,
    keyword_span: Span,
    predicate_span: Span,
    statements: Option<StatementsNode<'_>>,
    conditional_keyword: &str,
) {
    let Some(begin_node) =
        statements.and_then(|s| s.body().iter().next()).and_then(|n| n.as_begin_node())
    else {
        return;
    };
    let (Some(begin_kw), Some(end_kw)) =
        (begin_node.begin_keyword_loc(), begin_node.end_keyword_loc())
    else {
        return;
    };

    let condition_text = String::from_utf8_lossy(ctx.text(predicate_span)).into_owned();
    let column = ctx.line_col(node_span.start).column;
    let indent = " ".repeat(column as usize);
    let break_line = format!("break {conditional_keyword} {condition_text}\n{indent}");

    let fix = Fix {
        applicability: Applicability::Unsafe,
        edits: vec![
            Edit::replace(begin_kw.span(), b"loop do".to_vec()),
            Edit::insert(end_kw.span().start, break_line.into_bytes()),
            Edit::delete(Span::new(end_kw.span().end, node_span.end)),
        ],
    };
    ctx.report_with_fix(&Loop::META, keyword_span, MSG, fix);
}
