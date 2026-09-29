//! `Style/TrailingBodyOnModule`, ported from RuboCop's
//! `lib/rubocop/cop/style/trailing_body_on_module.rb` plus the
//! `TrailingBody` mixin and `LineBreakCorrector` it uses for
//! autocorrection.

use linter::{
    Applicability, CommentKind, Context, Department, Edit, Fix, FixAvailability, OptionError,
    OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Place the first line of module body on its own line.";

/// Checks for trailing code after the module definition.
#[derive(Debug, Clone)]
pub struct TrailingBodyOnModule {
    /// `Alignment#configured_indentation_width`: this cop has no
    /// `IndentationWidth` option of its own, so it always reads
    /// `Layout/IndentationWidth`'s `Width` (default 2).
    indentation_width: i64,
}

impl Rule for TrailingBodyOnModule {
    const META: RuleMeta = RuleMeta {
        name: "Style/TrailingBodyOnModule",
        department: Department::Style,
        summary: "Checks for trailing code after the module definition.",
        explanation: "\
```ruby
# bad
module Foo extend self
end

# good
module Foo
  extend self
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ModuleNode],
        config: &[],
        blind_spots: "\
`Layout/IndentationWidth`'s `Width` is read as a peer option, matching
upstream's own `Alignment#configured_indentation_width`.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .unwrap_or(2);
        Ok(Self { indentation_width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(module) = node.as_module_node() else { return };
        let Some(body) = module.body() else { return };

        // `node.multiline?`: the whole `module ... end` spans more than one line.
        let full_span = node.span();
        if ctx.is_single_line(full_span) {
            return;
        }

        // `body.begin_type? ? body.children.first : body`: Prism always wraps a
        // non-empty body in a `StatementsNode`, even a single statement, so the
        // "first part" is simply its first child either way.
        let Some(first_stmt) =
            body.as_statements_node().map_or(Some(body), |s| s.body().iter().next())
        else {
            return;
        };

        let keyword_span = module.module_keyword_loc().span();
        let keyword_line = ctx.line_col(keyword_span.start).line;
        let first_span = first_stmt.span();

        // `body_on_first_line?`: the body's first line matches the `module`
        // keyword's line -- i.e. the body trails on the header's own line.
        if ctx.line_col(first_span.start).line != keyword_line {
            return;
        }

        let keyword_column = i64::from(ctx.line_col(keyword_span.start).column);
        let mut edits = Vec::with_capacity(3);

        // `LineBreakCorrector.remove_semicolon`: the single `;` (if any)
        // separating the module header from its trailing body -- scanning
        // between the constant path and the body's first statement finds
        // exactly that separator, never a semicolon nested inside the body.
        let header_end = module.constant_path().span().end;
        if let Some(offset) =
            ctx.text(Span::new(header_end, first_span.start)).iter().position(|&b| b == b';')
        {
            let pos = header_end + u32::try_from(offset).expect("offset exceeds u32");
            edits.push(Edit::delete(Span::new(pos, pos + 1)));
        }

        // `LineBreakCorrector.move_comment`: an end-of-line comment on the
        // module's own line is hoisted above the `module` keyword.
        if let Some(comment) =
            ctx.comments().iter().find(|c| c.kind == CommentKind::Inline && c.line == keyword_line)
        {
            let mut prefix = ctx.text(comment.span).to_vec();
            prefix.push(b'\n');
            prefix.extend(std::iter::repeat_n(b' ', usize::try_from(keyword_column).unwrap_or(0)));
            edits.push(Edit::insert(full_span.start, prefix));
            edits.push(Edit::delete(comment.span));
        }

        // `LineBreakCorrector.break_line_before`: push the trailing body onto
        // its own line, indented one step past the `module` keyword.
        let indent_width = usize::try_from(keyword_column + self.indentation_width).unwrap_or(0);
        let mut linebreak = Vec::with_capacity(1 + indent_width);
        linebreak.push(b'\n');
        linebreak.extend(std::iter::repeat_n(b' ', indent_width));
        edits.push(Edit::insert(first_span.start, linebreak));

        ctx.report_with_fix(
            &Self::META,
            first_span,
            MSG,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}
