//! `Lint/ElseLayout`, ported from RuboCop's `lib/rubocop/cop/lint/else_layout.rb`
//! plus the `Alignment` mixin it includes for autocorrection indentation.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ElseNode, IfNode, UnlessNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Odd `else` layout detected. Did you mean to use `elsif`?";

/// Checks for odd `else` block layout, like an expression sitting on the
/// same line as the `else` (or `unless`'s `else`) keyword.
#[derive(Debug, Clone)]
pub struct ElseLayout {
    /// `Alignment#configured_indentation_width`: this cop has no
    /// `IndentationWidth` option of its own, so it always reads
    /// `Layout/IndentationWidth`'s `Width` (default 2).
    indentation_width: i64,
}

impl ElseLayout {
    /// RuboCop's `on_if` plus `check`, specialised to the branch that
    /// carries a real `else` clause. `subsequent` is `None` (no `else` at
    /// all), an `elsif` continuation (another `IfNode`, checked
    /// independently when the traversal visits it), or the real `ElseNode`
    /// this `if`/`elsif` directly owns.
    ///
    /// `then_present` is `node.then?`; `keyword_span` is the `if`/`elsif`
    /// keyword's own span, used for `Alignment#indentation`'s column.
    fn check_else_branch(
        &self,
        ctx: &mut Context<'_>,
        then_present: bool,
        full_span: Span,
        keyword_span: Span,
        else_node: &ElseNode<'_>,
    ) {
        let body_len = else_node.statements().map_or(0, |s| s.body().len());

        // `node.then? && !node.else_branch&.begin_type?`: a `then` clause
        // whose `else` body is a single statement (or empty) is the
        // intentional single-line style; skip the whole check.
        if then_present && body_len <= 1 {
            return;
        }

        // `node.single_line?`: the whole `if`/`unless` fits on one line,
        // deferred to `Style/OneLineConditional`.
        if ctx.is_single_line(full_span) {
            return;
        }

        // `else_branch.begin_type? ? else_branch.children.first : else_branch`
        let Some(first) = else_node.statements().and_then(|s| s.body().iter().next()) else {
            return;
        };

        let else_keyword_span = else_node.else_keyword_loc().span();
        let else_line = ctx.line_col(else_keyword_span.start).line;
        let first_span = first.span();
        if ctx.line_col(first_span.start).line != else_line {
            return;
        }

        let keyword_column = i64::from(ctx.line_col(keyword_span.start).column);
        let indent_width = usize::try_from(keyword_column + self.indentation_width).unwrap_or(0);
        let mut replacement = Vec::with_capacity(1 + indent_width);
        replacement.push(b'\n');
        replacement.extend(std::iter::repeat_n(b' ', indent_width));

        ctx.report_with_fix(
            &<Self as Rule>::META,
            first_span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(
                    Span::new(else_keyword_span.end, first_span.start),
                    replacement,
                )],
            },
        );
    }

    /// RuboCop's `on_if`/`check` for an `IfNode`: skips ternaries (no
    /// `if`/`elsif` keyword) and defers to `check_else_branch` only when
    /// this node's own `subsequent` is a real `else` clause, not another
    /// `elsif` link (that link is visited, and checked, independently).
    fn check_if(&self, ctx: &mut Context<'_>, node: &IfNode<'_>) {
        let Some(keyword_loc) = node.if_keyword_loc() else { return };
        let Some(subsequent) = node.subsequent() else { return };
        let Some(else_node) = subsequent.as_else_node() else { return };
        self.check_else_branch(
            ctx,
            node.then_keyword_loc().is_some(),
            node.location().span(),
            keyword_loc.span(),
            &else_node,
        );
    }

    /// RuboCop's `on_if`/`check` for an `unless` statement (whitequark
    /// models `unless` as the same `:if` node type as `if`, so the
    /// original cop's `on_if` fires for it too; Prism gives `unless` its
    /// own node type instead, with a direct `else_clause` -- `unless` can
    /// never have an `elsif`-style continuation).
    fn check_unless(&self, ctx: &mut Context<'_>, node: &UnlessNode<'_>) {
        let Some(else_node) = node.else_clause() else { return };
        self.check_else_branch(
            ctx,
            node.then_keyword_loc().is_some(),
            node.location().span(),
            node.keyword_loc().span(),
            &else_node,
        );
    }
}

impl Rule for ElseLayout {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ElseLayout",
        department: Department::Lint,
        summary: "Checks for odd code arrangement in an else block.",
        explanation: "\
Checks for odd `else` block layout -- like having an expression on the same
line as the `else` keyword, which is usually a mistake.

Its autocorrection tweaks layout to keep the syntax. So, this autocorrection
is compatible correction for bad case syntax, but if your code makes a
mistake with `elsif` and `else`, you will have to correct it manually.

```ruby
# bad

if something
  # ...
else do_this
  do_that
end

# good

# This code is compatible with the bad case. It will be autocorrected like
# this.
if something
  # ...
else
  do_this
  do_that
end

# This code is incompatible with the bad case.
# If `do_this` is a condition, `elsif` should be used instead of `else`.
if something
  # ...
elsif do_this
  do_that
end

# bad

# For single-line conditionals using `then` the layout is disallowed
# when the `else` body is multiline because it is treated as a lint offense.
if something then on_the_same_line_as_then
else first_line
  second_line
end

# good

# For single-line conditional using `then` the layout is allowed
# when `else` body is a single-line because it is treated as intentional.

if something then on_the_same_line_as_then
else single_line
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[],
        blind_spots: "\
`unless`'s `else` is checked (whitequark's parser models `unless` as the
same `:if` node type RuboCop's `on_if` dispatches on); Prism gives it a
distinct `UnlessNode`, handled the same way here. An `elsif` chain is
walked one link at a time as the traversal visits each nested `IfNode`
rather than through RuboCop's explicit recursive `check`, which is
observationally identical (RuboCop's own offense-location dedup is why a
naive reading of its recursion does not double-report).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .unwrap_or(2);
        Ok(Self { indentation_width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::IfNode { .. } => {
                let n = node.as_if_node().expect("kind matched");
                self.check_if(ctx, &n);
            }
            Node::UnlessNode { .. } => {
                let n = node.as_unless_node().expect("kind matched");
                self.check_unless(ctx, &n);
            }
            _ => {}
        }
    }
}
