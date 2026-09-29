//! `Style/TrailingBodyOnClass`, ported from RuboCop's
//! `lib/rubocop/cop/style/trailing_body_on_class.rb` plus the `TrailingBody`
//! mixin (`lib/rubocop/cop/mixin/trailing_body.rb`) and the
//! `LineBreakCorrector` (`lib/rubocop/cop/correctors/line_break_corrector.rb`)
//! its autocorrection uses.
//!
//! Prism always wraps a class/singleton-class body in a
//! [`NodeKind::StatementsNode`], even when it holds exactly one statement --
//! unlike whitequark's AST, which leaves a lone statement unwrapped (only
//! wrapping several statements in a synthetic `begin` node,
//! `body.begin_type?`). `TrailingBody#first_part_of` special-cases that
//! split (`begin_type?` takes the first child's range, otherwise the whole
//! body's range), but both branches land on the same span here: the body's
//! first statement, uniformly reached through the `StatementsNode`.

use linter::{
    Applicability, CommentInfo, Context, Department, Edit, Fix, FixAvailability, OptionError,
    OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Place the first line of class body on its own line.";

/// Class body goes below class statement.
#[derive(Debug, Clone)]
pub struct TrailingBodyOnClass {
    /// `Alignment#configured_indentation_width`: this cop has no
    /// `IndentationWidth` option of its own, so it always reads
    /// `Layout/IndentationWidth`'s `Width` (default 2).
    indentation_width: i64,
}

impl Rule for TrailingBodyOnClass {
    const META: RuleMeta = RuleMeta {
        name: "Style/TrailingBodyOnClass",
        department: Department::Style,
        summary: "Class body goes below class statement.",
        explanation: "\
```ruby
# bad
class Foo; def foo; end
end

# good
class Foo
  def foo; end
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode, NodeKind::SingletonClassNode],
        config: &[],
        blind_spots: "\
`Layout/IndentationWidth`'s `Width` is read as a peer option (falling back \
to 2), matching upstream's `Alignment#configured_indentation_width`.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .unwrap_or(2);
        Ok(Self { indentation_width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (keyword_span, body) = match node {
            Node::ClassNode { .. } => {
                let class = node.as_class_node().expect("kind matched");
                (class.class_keyword_loc().span(), class.body())
            }
            Node::SingletonClassNode { .. } => {
                let sclass = node.as_singleton_class_node().expect("kind matched");
                (sclass.class_keyword_loc().span(), sclass.body())
            }
            _ => return,
        };
        let Some(first_stmt) =
            body.and_then(|b| b.as_statements_node()).and_then(|s| s.body().first())
        else {
            return;
        };

        let node_span = node.span();
        // RuboCop's `node.multiline?`.
        if ctx.is_single_line(node_span) {
            return;
        }
        let node_line = ctx.line_col(node_span.start).line;
        let offense_span = first_stmt.span();
        // RuboCop's `body_on_first_line?` (`same_line?(node, body)`).
        if ctx.line_col(offense_span.start).line != node_line {
            return;
        }

        let fix = self.build_fix(ctx, node_span, keyword_span, offense_span, node_line);
        ctx.report_with_fix(&Self::META, offense_span, MSG, fix);
    }
}

impl TrailingBodyOnClass {
    /// RuboCop's `LineBreakCorrector.correct_trailing_body`.
    fn build_fix(
        &self,
        ctx: &Context<'_>,
        node_span: Span,
        keyword_span: Span,
        offense_span: Span,
        node_line: u32,
    ) -> Fix {
        let keyword_column = ctx.line_col(keyword_span.start).column;
        let width = usize::try_from(self.indentation_width).unwrap_or(2);
        let mut edits = Vec::with_capacity(3);

        // `break_line_before`.
        let indent = usize::try_from(keyword_column).unwrap_or(0) + width;
        edits.push(Edit::insert(
            offense_span.start,
            format!("\n{}", " ".repeat(indent)).into_bytes(),
        ));

        // `move_comment`.
        if let Some(comment) = eol_comment(ctx, node_line) {
            let text = String::from_utf8_lossy(ctx.text(comment.span)).into_owned();
            let spaces = " ".repeat(usize::try_from(keyword_column).unwrap_or(0));
            edits.push(Edit::insert(node_span.start, format!("{text}\n{spaces}").into_bytes()));
            edits.push(Edit::delete(comment.span));
        }

        // `remove_semicolon`.
        if let Some(semi) = find_semicolon(ctx, node_span.start, offense_span.start) {
            edits.push(Edit::delete(semi));
        }

        Fix { applicability: Applicability::Safe, edits }
    }
}

/// RuboCop's `processed_source.comment_at_line(node.source_range.line)`.
fn eol_comment(ctx: &Context<'_>, line: u32) -> Option<CommentInfo> {
    ctx.comments().iter().copied().find(|c| c.line == line)
}

/// RuboCop's `semicolon`/`trailing_class_definition?`: the first `;` between
/// the class node's own start and its body's first statement, skipping
/// string/comment bytes.
fn find_semicolon(ctx: &Context<'_>, start: u32, end: u32) -> Option<Span> {
    let bytes = ctx.text(Span::new(start, end));
    for (i, &byte) in bytes.iter().enumerate() {
        if byte != b';' {
            continue;
        }
        let offset = start + u32::try_from(i).expect("offset within node span fits u32");
        if !ctx.in_opaque_span(offset) {
            return Some(Span::new(offset, offset + 1));
        }
    }
    None
}
