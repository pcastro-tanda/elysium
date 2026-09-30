//! `Layout/ClosingParenthesisIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/closing_parenthesis_indentation.rb` plus the
//! `Alignment` mixin (`lib/rubocop/cop/mixin/alignment.rb`) and
//! `AlignmentCorrector` it uses for autocorrection.
//!
//! RuboCop's `on_send`/`on_begin`/`on_def` each hand `check` a `(node,
//! elements)` pair: the node whose own `loc.begin`/`loc.end` are the
//! opening/closing parenthesis, and the enumerable of "arguments" to judge
//! alignment/indentation against. Prism has no single wrapper type playing
//! that role for every case, so [`ClosingParenthesisIndentation::enter`]
//! reads the three shapes directly off their own node kind:
//! - `CallNode` (`on_send`/`on_csend`): `opening_loc`/`closing_loc` are the
//!   call's own argument-list parens; `elements` is the raw argument list
//!   (a bare keyword tail is already one [`NodeKind::KeywordHashNode`]
//!   element, matching whitequork's `hash`-typed last argument).
//! - `DefNode` (`on_def`/`on_defs`): `lparen_loc`/`rparen_loc` are the
//!   parameter list's own parens; `elements` is every parameter, flattened
//!   in declaration order (RuboCop-AST's `ArgsNode` acting as an
//!   `Enumerable` over its children).
//! - `ParenthesesNode` (`on_begin`, RuboCop's grouped-expression case):
//!   whitequork always wraps an explicit `(...)` group in a `:begin` node
//!   even for a single statement (unlike the generic method/block-body
//!   elision Prism's `StatementsNode` wrapper otherwise papers over), so
//!   this node kind alone reproduces `on_begin` faithfully; `elements` is
//!   the statement list inside. A `def` body's *implicit* `begin` (no
//!   parens at all) never surfaces here since it is not a `ParenthesesNode`,
//!   matching upstream's `return unless right_paren` guard on a `nil`
//!   `loc.end`.
//!
//! `all_elements_aligned?`'s hash branch (checking a lone leading hash
//! argument's own pairs' columns rather than the whole argument list's)
//! reuses `Layout/HashAlignment`'s `hash_elements`/`as_hash_like` helpers
//! verbatim -- the same braced-`HashNode`-vs-braceless-`KeywordHashNode`
//! split applies here.
//!
//! `CallNode`'s `opening_loc`/`closing_loc` are also used for `a[1]`'s
//! `[`/`]` (index read/write shares the same struct fields as a
//! parenthesized argument list in Prism); [`is_round_parens`] filters a
//! `CallNode` to a literal single-byte `(`/`)` pair before checking it,
//! excluding index calls the same way whitequork's own `Send`/`CSend`
//! `loc.begin`/`loc.end` map (which real RuboCop's `on_send` reads) never
//! populates for one.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ParametersNode, ParenthesesNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::hash_alignment::{as_hash_like, hash_elements};

/// Checks the indentation of hanging closing parentheses, ported from
/// RuboCop's `ClosingParenthesisIndentation` cop plus its `Alignment`
/// mixin and `AlignmentCorrector`.
#[derive(Debug, Clone)]
pub struct ClosingParenthesisIndentation {
    /// RuboCop's `Alignment#configured_indentation_width`.
    indentation_width: i64,
}

impl Rule for ClosingParenthesisIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/ClosingParenthesisIndentation",
        department: Department::Layout,
        summary: "Checks the indentation of hanging closing parentheses.",
        explanation: "\
Checks the indentation of hanging closing parentheses in
method calls, method definitions, and grouped expressions. A hanging
closing parenthesis means `)` preceded by a line break.

```ruby
# bad
some_method(
  a,
  b
  )

# good: when first param is on a new line, right paren is *always*
#       outdented by IndentationWidth
some_method(
  a,
  b
)

# good: when all other params are also on the same line, outdent
#       right paren by IndentationWidth
some_method(a, b, c
           )

# good: when all other params are on multiple lines, but are lined
#       up, align right paren with left paren
some_method(a,
            b,
            c
           )

# good: when other params are not lined up on multiple lines, outdent
#       right paren by IndentationWidth
some_method(a,
  x: 1,
  y: 2
)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::DefNode, NodeKind::ParenthesesNode],
        config: &[ConfigOption {
            name: "IndentationWidth",
            default: ConfigDefault::Nil,
            allowed: &[],
            doc: "Overrides `Layout/IndentationWidth`'s configured width for \
                  the outdented-closing-paren column; falls back to it, else 2.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let indentation_width = options
            .get("IndentationWidth")
            .and_then(OptionValue::as_int)
            .or_else(|| {
                options.peer("Layout/IndentationWidth", "Width").and_then(OptionValue::as_int)
            })
            .unwrap_or(2);
        Ok(Self { indentation_width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                let Some(left) = call.opening_loc() else { return };
                let Some(right) = call.closing_loc() else { return };
                if !is_round_parens(ctx, left.span(), right.span()) {
                    return;
                }
                let elements: Vec<Node<'_>> = call
                    .arguments()
                    .map(|args| args.arguments().iter().collect())
                    .unwrap_or_default();
                let node_col = i64::from(ctx.display_column(node.span().start));
                self.check(ctx, left.span(), right.span(), &elements, node_col);
            }
            Node::DefNode { .. } => {
                let def = node.as_def_node().expect("kind matched");
                let Some(left) = def.lparen_loc() else { return };
                let Some(right) = def.rparen_loc() else { return };
                let elements = def_parameter_list(def.parameters());
                let node_col = i64::from(ctx.display_column(left.span().start));
                self.check(ctx, left.span(), right.span(), &elements, node_col);
            }
            Node::ParenthesesNode { .. } => {
                let parens = node.as_parentheses_node().expect("kind matched");
                let left = parens.opening_loc();
                let right = parens.closing_loc();
                let elements = paren_children(&parens);
                let node_col = i64::from(ctx.display_column(left.span().start));
                self.check(ctx, left.span(), right.span(), &elements, node_col);
            }
            _ => {}
        }
    }
}

impl ClosingParenthesisIndentation {
    /// RuboCop's `check`.
    fn check(
        &self,
        ctx: &mut Context<'_>,
        left: Span,
        right: Span,
        elements: &[Node<'_>],
        node_col: i64,
    ) {
        if elements.is_empty() {
            Self::check_for_no_elements(ctx, left, right, node_col);
        } else {
            self.check_for_elements(ctx, left, right, elements);
        }
    }

    /// RuboCop's `check_for_elements`.
    fn check_for_elements(
        &self,
        ctx: &mut Context<'_>,
        left: Span,
        right: Span,
        elements: &[Node<'_>],
    ) {
        if !ctx.begins_its_line(right) {
            return;
        }
        let left_col = i64::from(ctx.display_column(left.start));
        let correct_column = self.expected_column(ctx, left, left_col, elements);
        let right_col = i64::from(ctx.display_column(right.start));
        let column_delta = correct_column - right_col;
        if column_delta == 0 {
            return;
        }
        register(ctx, right, correct_column, left_col, right_col, column_delta);
    }

    /// RuboCop's `check_for_no_elements`.
    fn check_for_no_elements(ctx: &mut Context<'_>, left: Span, right: Span, node_col: i64) {
        if !ctx.begins_its_line(right) {
            return;
        }
        let left_col = i64::from(ctx.display_column(left.start));
        let left_line = ctx.line_col(left.start).line;
        let candidates = [i64::from(indentation_of_line(ctx, left_line)), left_col, node_col];
        let right_col = i64::from(ctx.display_column(right.start));
        if candidates.contains(&right_col) {
            return;
        }
        let correct_column = candidates[0];
        let column_delta = correct_column - right_col;
        register(ctx, right, correct_column, left_col, right_col, column_delta);
    }

    /// RuboCop's `expected_column`.
    fn expected_column(
        &self,
        ctx: &Context<'_>,
        left: Span,
        left_col: i64,
        elements: &[Node<'_>],
    ) -> i64 {
        let left_line = ctx.line_col(left.start).line;
        let first_line = ctx.line_col(elements[0].span().start).line;
        if first_line > left_line {
            let source_indent = i64::from(indentation_of_line(ctx, first_line));
            let new_indent = source_indent - self.indentation_width;
            new_indent.max(0)
        } else if all_elements_aligned(ctx, elements) {
            left_col
        } else {
            i64::from(indentation_of_line(ctx, first_line))
        }
    }
}

/// RuboCop's `message`.
fn message(correct_column: i64, left_col: i64, right_col: i64) -> String {
    if correct_column == left_col {
        "Align `)` with `(`.".to_string()
    } else {
        format!("Indent `)` to column {correct_column} (not {right_col})")
    }
}

/// RuboCop's `add_offense` block plus `autocorrect` (`AlignmentCorrector.correct`
/// applied to the closing-paren range alone).
fn register(
    ctx: &mut Context<'_>,
    right: Span,
    correct_column: i64,
    left_col: i64,
    right_col: i64,
    column_delta: i64,
) {
    let msg = message(correct_column, left_col, right_col);
    match build_fix(ctx, right, column_delta) {
        Some(fix) => ctx.report_with_fix(&ClosingParenthesisIndentation::META, right, msg, fix),
        None => ctx.report(&ClosingParenthesisIndentation::META, right, msg),
    }
}

/// RuboCop's `AlignmentCorrector.correct(corrector, processed_source, node,
/// @column_delta)` where `node` is the closing-paren range itself: shifts
/// that one physical line by `column_delta` columns. No heredoc taboo is
/// needed since the shifted span never spans more than the single line the
/// closing delimiter begins.
fn build_fix(ctx: &Context<'_>, right: Span, column_delta: i64) -> Option<Fix> {
    let delta = i32::try_from(column_delta).unwrap_or(0);
    let edits = linter::shift_lines(ctx, right, delta, &[]);
    if edits.is_empty() {
        None
    } else {
        Some(Fix { applicability: Applicability::Safe, edits })
    }
}

/// RuboCop's `all_elements_aligned?`: a lone leading hash argument's own
/// pairs' columns when the first element is hash-shaped (covers both a
/// braced `HashNode` and a braceless `KeywordHashNode`), else every
/// element's own start column; `true` only when every such column is
/// identical and there is at least one (`Array#uniq.one?`, which is
/// `false` on an empty array).
fn all_elements_aligned(ctx: &Context<'_>, elements: &[Node<'_>]) -> bool {
    let columns: Vec<u32> = if as_hash_like(&elements[0]).is_some() {
        hash_elements(&elements[0]).iter().map(|c| ctx.display_column(c.span().start)).collect()
    } else {
        elements.iter().map(|e| ctx.display_column(e.span().start)).collect()
    };
    let Some(&first) = columns.first() else { return false };
    columns.iter().all(|&c| c == first)
}

/// RuboCop-AST's `ArgsNode` acting as an `Enumerable` over its children:
/// every entry in a `def`'s parameter list, in declaration order.
fn def_parameter_list(params: Option<ParametersNode<'_>>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    let Some(params) = params else { return out };
    out.extend(params.requireds().iter());
    out.extend(params.optionals().iter());
    if let Some(rest) = params.rest() {
        out.push(rest);
    }
    out.extend(params.posts().iter());
    out.extend(params.keywords().iter());
    if let Some(kwrest) = params.keyword_rest() {
        out.push(kwrest);
    }
    if let Some(block) = params.block() {
        out.push(block.as_node());
    }
    out
}

/// A `CallNode`'s `opening_loc`/`closing_loc` are also used for `a[1]`'s
/// `[`/`]` (index read/write share the same struct fields as a
/// parenthesized argument list in Prism), which whitequork's `Send`/`CSend`
/// node's `loc.begin`/`loc.end` map never populates for an index call --
/// real RuboCop's own `on_send` therefore never reaches `check` for one.
/// Filtering to a literal single-byte `(`/`)` pair (mirroring
/// `SpaceInsideParens`'s own such filter) excludes index calls for free.
fn is_round_parens(ctx: &Context<'_>, left: Span, right: Span) -> bool {
    matches!(ctx.text(left), [b'(']) && matches!(ctx.text(right), [b')'])
}

/// RuboCop's `on_begin`'s `node.children`: a `ParenthesesNode`'s inner
/// statement list, or empty for `()`. Prism always wraps the body in a
/// `StatementsNode`, even for a single statement -- unlike a `def`/block
/// body, whitequark itself never elides the `:begin` wrapper for an
/// explicit parenthesized group regardless of statement count, so this
/// node kind alone reproduces `on_begin` faithfully.
fn paren_children<'pr>(paren: &ParenthesesNode<'pr>) -> Vec<Node<'pr>> {
    paren
        .body()
        .and_then(|body| body.as_statements_node())
        .map(|stmts| stmts.body().iter().collect())
        .unwrap_or_default()
}

/// RuboCop's `processed_source.line_indentation`: the character column of
/// the first non-whitespace byte on `line`, or `0` for a blank line.
/// Leading indentation is always plain ASCII spaces/tabs, so a byte
/// position here is already the matching character column.
fn indentation_of_line(ctx: &Context<'_>, line: u32) -> u32 {
    let text = ctx.line_text(line);
    text.iter()
        .position(|&b| !b.is_ascii_whitespace())
        .map_or(0, |pos| u32::try_from(pos).unwrap_or(u32::MAX))
}
