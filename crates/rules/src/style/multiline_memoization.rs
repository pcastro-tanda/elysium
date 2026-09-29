//! `Style/MultilineMemoization`, ported from RuboCop's
//! `lib/rubocop/cop/style/multiline_memoization.rb`.
//!
//! Whitequark's `or_asgn` fires for *every* `||=` regardless of the
//! assignment target's shape (local/instance/class/global variable,
//! constant, constant path, attribute, index). Prism instead fuses the
//! target's kind straight into the node kind (`LocalVariableOrWriteNode`,
//! `CallOrWriteNode`, ...), so this port subscribes to all eight `*OrWrite`
//! kinds and reads each one's `value()` as the right-hand side.
//!
//! Whitequark's `(...)` always wraps its content -- even a single
//! expression -- in a `begin`-type node; Prism's equivalent is
//! `ParenthesesNode` (`opening_loc`/`closing_loc`), used unconditionally for
//! parenthesized content regardless of statement count. Whitequark's
//! `kwbegin`-type explicit `begin ... end` keyword block is Prism's
//! `BeginNode` (its `begin_keyword_loc`/`end_keyword_loc` fields).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::BeginNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const KEYWORD_MSG: &str = "Wrap multiline memoization blocks in `begin` and `end`.";
const BRACES_MSG: &str = "Wrap multiline memoization blocks in `(` and `)`.";

/// Wrap multiline memoizations in a `begin` and `end` block.
#[derive(Debug, Clone)]
pub struct MultilineMemoization {
    /// `EnforcedStyle: braces` when `true`, else the default `keyword`.
    braces_style: bool,
    /// `Alignment#configured_indentation_width`: this cop has no
    /// `IndentationWidth` option of its own, so it always reads
    /// `Layout/IndentationWidth`'s `Width` (default 2).
    indentation_width: i64,
}

impl Rule for MultilineMemoization {
    const META: RuleMeta = RuleMeta {
        name: "Style/MultilineMemoization",
        department: Department::Style,
        summary: "Wrap multiline memoizations in a `begin` and `end` block.",
        explanation: "\
Checks expressions wrapping styles for multiline memoization.\n\n\
# EnforcedStyle: keyword (default)\n\n\
```ruby\n\
# bad\n\
foo ||= (\n\
  bar\n\
  baz\n\
)\n\n\
# good\n\
foo ||= begin\n\
  bar\n\
  baz\n\
end\n\
```\n\n\
# EnforcedStyle: braces\n\n\
```ruby\n\
# bad\n\
foo ||= begin\n\
  bar\n\
  baz\n\
end\n\n\
# good\n\
foo ||= (\n\
  bar\n\
  baz\n\
)\n\
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::LocalVariableOrWriteNode,
            NodeKind::InstanceVariableOrWriteNode,
            NodeKind::ClassVariableOrWriteNode,
            NodeKind::GlobalVariableOrWriteNode,
            NodeKind::ConstantOrWriteNode,
            NodeKind::ConstantPathOrWriteNode,
            NodeKind::CallOrWriteNode,
            NodeKind::IndexOrWriteNode,
        ],
        config: &[linter::ConfigOption {
            name: "EnforcedStyle",
            default: linter::ConfigDefault::Str("keyword"),
            allowed: &["keyword", "braces"],
            doc: "Whether to wrap multiline memoization blocks in `begin`/`end` \
keywords or in `(`/`)` parentheses.",
        }],
        blind_spots: "\
`Layout/IndentationWidth`'s `Width` is read as a peer option, matching \
upstream's own `Alignment#configured_indentation_width` cross-cop read.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let braces_style = options.style("EnforcedStyle")? == "braces";
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(linter::OptionValue::as_int)
            .unwrap_or(2);
        Ok(Self { braces_style, indentation_width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(rhs) = value_of(node) else { return };

        if !bad_rhs(&rhs, self.braces_style, ctx) {
            return;
        }

        let message = if self.braces_style { BRACES_MSG } else { KEYWORD_MSG };
        let span = first_line_span(node.span(), ctx);

        let fix = if self.braces_style {
            braces_fix(&rhs)
        } else {
            keyword_fix(&rhs, self.indentation_width, ctx)
        };

        match fix {
            Some(fix) => ctx.report_with_fix(&Self::META, span, message, fix),
            None => ctx.report(&Self::META, span, message),
        }
    }
}

/// The right-hand side (`value`) of any of the eight `*OrWriteNode` kinds.
fn value_of<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    Some(match node.kind() {
        NodeKind::LocalVariableOrWriteNode => node.as_local_variable_or_write_node()?.value(),
        NodeKind::InstanceVariableOrWriteNode => node.as_instance_variable_or_write_node()?.value(),
        NodeKind::ClassVariableOrWriteNode => node.as_class_variable_or_write_node()?.value(),
        NodeKind::GlobalVariableOrWriteNode => node.as_global_variable_or_write_node()?.value(),
        NodeKind::ConstantOrWriteNode => node.as_constant_or_write_node()?.value(),
        NodeKind::ConstantPathOrWriteNode => node.as_constant_path_or_write_node()?.value(),
        NodeKind::CallOrWriteNode => node.as_call_or_write_node()?.value(),
        NodeKind::IndexOrWriteNode => node.as_index_or_write_node()?.value(),
        _ => return None,
    })
}

/// RuboCop's `bad_rhs?`.
fn bad_rhs(rhs: &Node<'_>, braces_style: bool, ctx: &Context<'_>) -> bool {
    if ctx.is_single_line(rhs.span()) {
        return false;
    }

    if braces_style {
        rhs.as_begin_node().is_some_and(|begin| !contains_rescue_or_ensure(&begin))
    } else {
        rhs.kind() == NodeKind::ParenthesesNode
    }
}

/// RuboCop's `contains_rescue_or_ensure?`.
fn contains_rescue_or_ensure(begin: &BeginNode<'_>) -> bool {
    begin.rescue_clause().is_some() || begin.ensure_clause().is_some()
}

/// RuboCop's default `add_offense(node)` range, clamped to `node`'s first
/// physical line (matches upstream's `expect_offense` fixtures: the range
/// never extends past the end of the line it starts on).
fn first_line_span(span: Span, ctx: &Context<'_>) -> Span {
    let first_line = ctx.line_col(span.start).line;
    let line_end = ctx.line_span(first_line).end;
    Span::new(span.start, span.end.min(line_end))
}

/// RuboCop's braces-style autocorrect: `corrector.replace(rhs.loc.begin,
/// '(')` / `corrector.replace(rhs.loc.end, ')')`, a plain in-place swap of
/// the `begin`/`end` keywords for parentheses.
fn braces_fix(rhs: &Node<'_>) -> Option<Fix> {
    let begin = rhs.as_begin_node()?;
    let begin_kw = begin.begin_keyword_loc()?.span();
    let end_kw = begin.end_keyword_loc()?.span();
    Some(Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::replace(begin_kw, b"(".to_vec()), Edit::replace(end_kw, b")".to_vec())],
    })
}

/// RuboCop's keyword-style `keyword_autocorrect`: replaces the parens'
/// `(`/`)` with `begin`/`end`, reformatting onto their own lines only when
/// the parens weren't already alone on their line.
fn keyword_fix(rhs: &Node<'_>, indentation_width: i64, ctx: &Context<'_>) -> Option<Fix> {
    let parens = rhs.as_parentheses_node()?;
    let opening = parens.opening_loc().span();
    let closing = parens.closing_loc().span();

    let column = ctx.line_col(rhs.span().start).column;

    let begin_str = if ctx.text(Span::new(opening.end, opening.end + 1)) == b"\n" {
        b"begin".to_vec()
    } else {
        let indent = usize::try_from(i64::from(column) + indentation_width).unwrap_or(0);
        let mut s = b"begin\n".to_vec();
        s.extend(std::iter::repeat_n(b' ', indent));
        s
    };

    let end_line = ctx.line_col(closing.start).line;
    let end_line_text = ctx.line_text(end_line);
    let has_other_content =
        end_line_text.iter().any(|&b| !(b as char).is_whitespace() && b != b')');
    let end_str = if has_other_content {
        let indent = usize::try_from(column).unwrap_or(0);
        let mut s = b"\n".to_vec();
        s.extend(std::iter::repeat_n(b' ', indent));
        s.extend_from_slice(b"end");
        s
    } else {
        b"end".to_vec()
    };

    Some(Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::replace(opening, begin_str), Edit::replace(closing, end_str)],
    })
}
