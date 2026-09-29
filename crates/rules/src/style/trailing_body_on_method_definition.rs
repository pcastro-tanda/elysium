//! `Style/TrailingBodyOnMethodDefinition`, ported from RuboCop's
//! `lib/rubocop/cop/style/trailing_body_on_method_definition.rb` plus the
//! `TrailingBody` mixin it includes and the `LineBreakCorrector` it uses for
//! autocorrection.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Place the first line of a multi-line method definition's body on its own line.";

/// Checks for trailing code after the method definition.
#[derive(Debug, Clone)]
pub struct TrailingBodyOnMethodDefinition {
    /// `Alignment#configured_indentation_width`: this cop has no
    /// `IndentationWidth` option of its own, so it always reads
    /// `Layout/IndentationWidth`'s `Width` (default 2).
    indentation_width: i64,
}

impl TrailingBodyOnMethodDefinition {
    /// RuboCop's `TrailingBody#first_part_of`: the first statement of a
    /// multi-statement body (Prism always wraps a `def` body in a
    /// `StatementsNode`, unlike whitequark's elided single-statement
    /// `begin`).
    ///
    /// An implicit `rescue`/`else`/`ensure` directly on the method (no
    /// explicit inner `begin`/`end`) makes `def.body()` a `BeginNode`
    /// with no `begin_keyword_loc`; Prism gives that synthetic node the
    /// *same* span as the enclosing `DefNode` itself (start included),
    /// rather than the span of its real content, so it cannot be used
    /// directly. Measure from its first real statement instead, falling
    /// back to the `rescue`/`else`/`ensure` keyword when there is no
    /// leading statement at all (e.g. `def foo; rescue; end`).
    fn first_part_of(body: &Node<'_>) -> Option<Span> {
        if let Some(stmts) = body.as_statements_node() {
            return stmts.body().first().map(|first| first.span());
        }
        if let Some(begin) = body.as_begin_node() {
            if let Some(stmts) = begin.statements() {
                return stmts.body().first().map(|first| first.span());
            }
            return begin
                .rescue_clause()
                .map(|r| r.keyword_loc().span())
                .or_else(|| begin.else_clause().map(|e| e.else_keyword_loc().span()))
                .or_else(|| begin.ensure_clause().map(|e| e.ensure_keyword_loc().span()));
        }
        Some(body.span())
    }

    /// RuboCop's `LineBreakCorrector.semicolon`: the first `;` token after
    /// the `def` node's start that sits before the body's own column --
    /// i.e. the semicolon that terminates the method signature, never one
    /// inside the body itself.
    fn signature_semicolon(ctx: &Context<'_>, node_start: u32, body_start: u32) -> Option<Span> {
        let window = Span::new(node_start, body_start);
        for offset in window.start..window.end {
            if ctx.in_opaque_span(offset) {
                continue;
            }
            if ctx.text(Span::new(offset, offset + 1)) == b";" {
                return Some(Span::new(offset, offset + 1));
            }
        }
        None
    }
}

impl Rule for TrailingBodyOnMethodDefinition {
    const META: RuleMeta = RuleMeta {
        name: "Style/TrailingBodyOnMethodDefinition",
        department: Department::Style,
        summary: "Checks for trailing code after the method definition.",
        explanation: "\
NOTE: It always accepts endless method definitions that are basically on the same line.\n\
\n\
# bad\n\
def some_method; do_stuff\n\
end\n\
\n\
def f(x); b = foo\n\
  b[c: x]\n\
end\n\
\n\
# good\n\
def some_method\n\
  do_stuff\n\
end\n\
\n\
def f(x)\n\
  b = foo\n\
  b[c: x]\n\
end\n\
\n\
def endless_method = do_stuff",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[],
        blind_spots: "\
`Layout/IndentationWidth`'s `Width` is read as a peer option, matching upstream's own\n\
`Alignment#configured_indentation_width` cross-cop read.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .unwrap_or(2);
        Ok(Self { indentation_width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let def = node.as_def_node().expect("kind matched");

        // `node.endless?`: an endless method's whole body sits after `=`,
        // never a "trailing body" in the sense this cop cares about.
        if def.equal_loc().is_some() {
            return;
        }

        // `trailing_body?`: a body must exist, ...
        let Some(body) = def.body() else { return };
        // ... the whole `def` must span multiple lines, ...
        if ctx.is_single_line(node.span()) {
            return;
        }
        let Some(first_part) = Self::first_part_of(&body) else { return };
        // ... and the body must start on the `def` keyword's own line.
        let def_line = ctx.line_col(node.span().start).line;
        if ctx.line_col(first_part.start).line != def_line {
            return;
        }

        let keyword_span = def.def_keyword_loc().span();
        let keyword_column = i64::from(ctx.line_col(keyword_span.start).column);
        let indent = usize::try_from(keyword_column + self.indentation_width).unwrap_or(0);

        // `LineBreakCorrector.break_line_before`.
        let mut break_text = Vec::with_capacity(1 + indent);
        break_text.push(b'\n');
        break_text.extend(std::iter::repeat_n(b' ', indent));
        let mut edits = vec![Edit::insert(first_part.start, break_text)];

        // `LineBreakCorrector.move_comment`: an end-of-line comment on the
        // `def` line is hoisted above the (now comment-free) `def`.
        if let Some(comment) =
            ctx.comments().iter().find(|c| ctx.line_col(c.span.start).line == def_line)
        {
            let comment_span = comment.span;
            let comment_text = ctx.text(comment_span).to_vec();
            let keyword_column_usize = usize::try_from(keyword_column).unwrap_or(0);
            let mut hoisted = comment_text;
            hoisted.push(b'\n');
            hoisted.extend(std::iter::repeat_n(b' ', keyword_column_usize));
            edits.push(Edit::insert(node.span().start, hoisted));
            edits.push(Edit::delete(comment_span));
        }

        // `LineBreakCorrector.remove_semicolon`.
        if let Some(semicolon) = Self::signature_semicolon(ctx, node.span().start, first_part.start)
        {
            edits.push(Edit::delete(semicolon));
        }

        ctx.report_with_fix(
            &Self::META,
            first_part,
            MSG,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}
