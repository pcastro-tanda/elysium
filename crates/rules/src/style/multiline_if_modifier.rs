//! `Style/MultilineIfModifier`, ported from RuboCop's
//! `lib/rubocop/cop/style/multiline_if_modifier.rb` (plus the `StatementModifier`
//! and `Alignment` mixins it includes).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Favor a normal %<keyword>s-statement over a modifier clause in a multiline \
statement.";

/// Checks for uses of `if`/`unless` modifiers with multiline bodies.
#[derive(Debug, Clone)]
pub struct MultilineIfModifier {
    /// `Alignment#configured_indentation_width`: `Layout/IndentationWidth`'s
    /// `Width`, else `2`.
    indentation_width: i64,
    /// Spans of nodes already reported this file (RuboCop's `ignore_node`):
    /// a node fully nested inside one of these is `part_of_ignored_node?`
    /// and is skipped, since its enclosing modifier will already rewrite it
    /// (as plain text) when corrected.
    ignored: Vec<Span>,
}

impl Rule for MultilineIfModifier {
    const META: RuleMeta = RuleMeta {
        name: "Style/MultilineIfModifier",
        department: Department::Style,
        summary: "Checks for uses of if/unless modifiers with multiple-lines bodies.",
        explanation: "\
Checks for uses of if/unless modifiers with multiple-lines bodies.

```ruby
# bad
{
  result: 'this should not happen'
} unless cond

# good
{ result: 'ok' } if cond
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[],
        blind_spots: "\
Reads `Layout/IndentationWidth`'s `Width` as a peer option \
(`Alignment#configured_indentation_width`).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .unwrap_or(2);
        Ok(Self { indentation_width, ignored: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::IfNode { .. } => self.check_if(node, ctx),
            Node::UnlessNode { .. } => self.check_unless(node, ctx),
            _ => {}
        }
    }
}

impl MultilineIfModifier {
    fn check_if(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.is_ignored(node.span()) {
            return;
        }
        let if_node = node.as_if_node().expect("kind matched");
        let Some(keyword_loc) = if_node.if_keyword_loc() else {
            // Ternary (`a ? b : c`): no `if` keyword, never modifier form.
            return;
        };
        if ctx.text(keyword_loc.span()) == b"elsif" {
            // `elsif` shares its outer chain's `end`, so `modifier_form?` is
            // always false for it upstream too.
            return;
        }
        if if_node.end_keyword_loc().is_some() {
            return;
        }
        let Some(statements) = if_node.statements() else { return };
        let condition = if_node.predicate();
        self.check(node, "if", &condition, &statements.as_node(), ctx);
    }

    fn check_unless(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.is_ignored(node.span()) {
            return;
        }
        let unless_node = node.as_unless_node().expect("kind matched");
        if unless_node.end_keyword_loc().is_some() {
            return;
        }
        let Some(statements) = unless_node.statements() else { return };
        let condition = unless_node.predicate();
        self.check(node, "unless", &condition, &statements.as_node(), ctx);
    }

    /// RuboCop's `part_of_ignored_node?`.
    fn is_ignored(&self, span: Span) -> bool {
        self.ignored.iter().any(|s| s.start <= span.start && span.end <= s.end)
    }

    /// RuboCop's `on_if`: `node.modifier_form? && node.body.multiline?`.
    fn check(
        &mut self,
        node: &Node<'_>,
        keyword: &'static str,
        condition: &Node<'_>,
        body: &Node<'_>,
        ctx: &mut Context<'_>,
    ) {
        if !is_body_multiline(ctx, body) {
            return;
        }
        let message = MSG.replace("%<keyword>s", keyword);
        let replacement = self.to_normal_if(ctx, node.span(), keyword, condition, body);
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node.span(), replacement.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, node.span(), message, fix);
        self.ignored.push(node.span());
    }

    /// RuboCop's `to_normal_if`.
    fn to_normal_if(
        &self,
        ctx: &Context<'_>,
        node_span: Span,
        keyword: &str,
        condition: &Node<'_>,
        body: &Node<'_>,
    ) -> String {
        let offset_len = ctx.line_col(node_span.start).column as usize;
        let offset = " ".repeat(offset_len);
        let width = usize::try_from(self.indentation_width).unwrap_or(2);
        let indentation = " ".repeat(offset_len + width);

        let condition_text = String::from_utf8_lossy(ctx.text(condition.span())).into_owned();
        let condition_line = format!("{keyword} {condition_text}");

        let body_text = String::from_utf8_lossy(ctx.text(body.span())).into_owned();
        let indented_body = Self::indented_body(&offset, &indentation, &body_text);

        let indented_end = format!("{offset}end");

        format!("{condition_line}\n{indented_body}\n{indented_end}")
    }

    /// RuboCop's `indented_body`: prefixes the body's own source with
    /// `offset`, then re-indents every non-blank line by replacing its
    /// first `offset.len()` whitespace bytes with `indentation`.
    fn indented_body(offset: &str, indentation: &str, body: &str) -> String {
        let body_source = format!("{offset}{body}");
        let mut result = String::with_capacity(body_source.len() + indentation.len());
        for line in split_keeping_newlines(&body_source) {
            if line == "\n" {
                result.push_str(line);
                continue;
            }
            match strip_leading_whitespace(line, offset.len()) {
                Some(rest) => {
                    result.push_str(indentation);
                    result.push_str(rest);
                }
                None => result.push_str(line),
            }
        }
        result
    }
}

/// RuboCop-AST's `Node#multiline?`, which `BlockNode`/`LambdaNode` override
/// from the generic first-to-last-line-of-the-whole-node check to just their
/// own opening/closing delimiter lines (`{`/`}` or `do`/`end`, or `->`'s
/// `(`/`)`/`{`/`}`). Without this, a modifier `if` whose body is a one-line
/// block call reached through a multi-line method chain (e.g.
/// `foo\n  .bar { baz } if cond`) would wrongly count as a multiline body.
fn is_body_multiline(ctx: &Context<'_>, body: &Node<'_>) -> bool {
    // `if_node.statements()` always wraps the body in a `StatementsNode`,
    // even for a single statement (unlike whitequark, which elides a
    // single-statement wrapper); unwrap it so the block/lambda checks
    // below see the actual statement, matching whitequark's `node.body`.
    let body = match body.as_statements_node() {
        Some(s) if s.body().len() == 1 => s.body().first().expect("len==1"),
        _ => *body,
    };
    if let Some(block) = body.as_call_node().and_then(|c| c.block()).and_then(|b| b.as_block_node())
    {
        return !ctx.same_line(block.opening_loc().span(), block.closing_loc().span());
    }
    if let Some(lambda) = body.as_lambda_node() {
        return !ctx.same_line(lambda.opening_loc().span(), lambda.closing_loc().span());
    }
    !ctx.is_single_line(body.span())
}

/// Ruby's `String#each_line`: splits after every `\n`, keeping it attached to
/// the preceding text; the final chunk has no trailing `\n` if the string
/// didn't end with one.
fn split_keeping_newlines(s: &str) -> impl Iterator<Item = &str> {
    s.split_inclusive('\n')
}

/// `line.sub(/^\s{n}/, ...)`'s match half: `Some(rest)` when `line`'s first
/// `n` bytes are all ASCII whitespace, `None` otherwise (n = 0 always
/// matches, at the empty prefix).
fn strip_leading_whitespace(line: &str, n: usize) -> Option<&str> {
    let bytes = line.as_bytes();
    if n > bytes.len() || !bytes[..n].iter().all(u8::is_ascii_whitespace) {
        return None;
    }
    Some(&line[n..])
}
