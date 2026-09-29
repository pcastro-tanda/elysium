//! `Style/TrailingMethodEndStatement`, ported from RuboCop's
//! `lib/rubocop/cop/style/trailing_method_end_statement.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "Place the end statement of a multi-line method on its own line.";

/// Checks for trailing code after the method definition.
#[derive(Debug, Clone)]
pub struct TrailingMethodEndStatement;

impl Rule for TrailingMethodEndStatement {
    const META: RuleMeta = RuleMeta {
        name: "Style/TrailingMethodEndStatement",
        department: Department::Style,
        summary: "Checks for trailing code after the method definition.",
        explanation: "Method bodies whose final statement shares a line with the closing \
            `end` are hard to read; the `end` should be on its own line.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let def = node.as_def_node().expect("kind matched");
        if def.equal_loc().is_some() {
            return; // endless method
        }
        let Some(body) = def.body() else { return };
        let Some(end_loc) = def.end_keyword_loc() else { return };
        if ctx.is_single_line(node.span()) {
            return;
        }
        if ctx.last_line(effective_body_span(&body)) != ctx.last_line(end_loc.span()) {
            return;
        }

        let end_span = end_loc.span();
        let indent_column = ctx.line_col(def.def_keyword_loc().span().start).column;
        let indent_column = usize::try_from(indent_column).expect("column fits in usize");
        let mut insertion = Vec::with_capacity(1 + indent_column);
        insertion.push(b'\n');
        insertion.extend(std::iter::repeat_n(b' ', indent_column));

        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::insert(end_span.start, insertion)],
        };
        ctx.report_with_fix(&Self::META, end_span, MSG, fix);
    }
}

/// The byte range whitequark's `body_and_end_on_same_line?` sees for
/// `node.children.last`. Prism gives an *implicit* `BeginNode` -- the one
/// standing for a `def` body that carries `rescue`/`else`/`ensure` clauses --
/// the span of the whole method body, `end` keyword included. whitequark's
/// equivalent `rescue`/`ensure` node covers only the clauses themselves, so
/// the span is rebuilt from the clause parts.
fn effective_body_span(node: &Node<'_>) -> ruby_source::Span {
    let Some(begin) = node.as_begin_node() else { return node.span() };
    if begin.begin_keyword_loc().is_some() {
        return node.span();
    }

    let statements = begin.statements().map(|s| s.as_node().span());
    let rescue = begin.rescue_clause().map(|r| r.as_node().span());
    let start = statements.or(rescue).map_or_else(|| node.span().start, |span| span.start);

    let mut end = statements.map_or(start, |span| span.end);
    if let Some(rescue) = rescue {
        end = end.max(rescue.end);
    }
    if let Some(else_clause) = begin.else_clause() {
        end =
            end.max(else_clause.statements().map_or_else(
                || else_clause.else_keyword_loc().span().end,
                |s| s.as_node().span().end,
            ));
    }
    if let Some(ensure) = begin.ensure_clause() {
        end = end.max(
            ensure
                .statements()
                .map_or_else(|| ensure.ensure_keyword_loc().span().end, |s| s.as_node().span().end),
        );
    }
    ruby_source::Span::new(start, end)
}
