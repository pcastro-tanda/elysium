//! `Layout/MultilineBlockLayout`, ported from RuboCop's
//! `lib/rubocop/cop/layout/multiline_block_layout.rb`.
//!
//! Upstream's `on_block`/`on_numblock`/`on_itblock` all see the same
//! whitequark `:block` node shape -- whitequark unifies a stabby-lambda
//! literal (`-> (x) do ... end`) into that same node type as an explicit
//! `do`/`{` block, with the same `loc.begin`/`arguments`/`body` API. Prism
//! instead gives a lambda literal its own [`ruby_ast::node::LambdaNode`],
//! separate from [`ruby_ast::node::CallNode`]'s attached
//! [`ruby_ast::node::BlockNode`], so [`BlockLike`] is filled from either
//! shape and every check below runs against that one common view.
//!
//! Prism also collapses whitequark's `:numblock`/`:itblock` distinction: a
//! numbered-parameter (`_1`) or `it`-parameter block is still just a
//! `BlockNode`, whose `parameters()` is a `NumberedParametersNode`/
//! `ItParametersNode` instead of a `BlockParametersNode`. RuboCop-AST's
//! `BlockNode#arguments` reports both of those as having no explicit
//! arguments ("Numblocks and itblocks have no explicit block arguments"),
//! so [`explicit_params`] excludes them the same way.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, LambdaNode, MultiTargetNode, ParametersNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{char_len, Side, Span};

/// RuboCop's `MSG`.
const MSG: &str = "Block body expression is on the same line as the block start.";
/// RuboCop's `ARG_MSG`.
const ARG_MSG: &str = "Block argument expression is not on the same line as the block start.";

/// Ensures newlines after multiline block do statements.
#[derive(Debug, Clone)]
pub struct MultilineBlockLayout {
    /// RuboCop's `max_line_length`: `Layout/LineLength`'s configured `Max`,
    /// `None` when that cop is disabled (`config.cop_enabled?` guard).
    max_line_length: Option<i64>,
}

/// The parts of a block-like construct this cop needs, filled from either a
/// `CallNode`'s attached `BlockNode` or a `LambdaNode` directly (see the
/// module doc).
struct BlockLike<'pr> {
    /// RuboCop's `node.source_range`: the whole construct's span, used for
    /// `node.source_range.column` (the indentation base for
    /// `autocorrect_body`/`needed_length_for_args`). A `CallNode`'s own
    /// location already covers its attached block through the closing
    /// `end`/`}`, including any multi-line method-chain receiver; a
    /// `LambdaNode`'s own location covers `->` through the same.
    whole: Span,
    /// RuboCop's `node.loc.begin`: the `do`/`{` opening delimiter.
    opening: Span,
    /// RuboCop's `node.loc.end`: the `end`/`}` closing delimiter.
    ///
    /// `BlockNode#single_line?` is overridden from the generic
    /// `Node#single_line?` (first-to-last line of the *whole* expression,
    /// which for a multi-line method-chain receiver like
    /// `foo\n  .bar { baz }` would wrongly call the block itself
    /// multi-line) to just `loc.begin.line == loc.end.line`: whether the
    /// opening and closing delimiters share a line, ignoring the receiver
    /// entirely. This is why `opening`/`closing` are tracked separately
    /// from `whole`.
    closing: Span,
    /// The explicit parameter list node (a `BlockParametersNode`, or a bare
    /// `ParametersNode` for a parenthesis-less stabby lambda), when one
    /// exists. `None` for no parameters at all, and for Prism's
    /// `NumberedParametersNode`/`ItParametersNode` implicit shapes (see the
    /// module doc).
    params: Option<Node<'pr>>,
    /// RuboCop's `node.body`'s span, when the block/lambda has a body.
    body: Option<Span>,
}

impl<'pr> BlockLike<'pr> {
    fn from_call(call: &CallNode<'pr>) -> Option<Self> {
        let block = call.block()?.as_block_node()?;
        Some(Self {
            whole: call.as_node().span(),
            opening: block.opening_loc().span(),
            closing: block.closing_loc().span(),
            params: block.parameters().filter(explicit_params),
            body: block.body().map(|b| effective_body_span(&b)),
        })
    }

    fn from_lambda(lambda: &LambdaNode<'pr>) -> Self {
        Self {
            whole: lambda.as_node().span(),
            opening: lambda.opening_loc().span(),
            closing: lambda.closing_loc().span(),
            params: lambda.parameters().filter(explicit_params),
            body: lambda.body().map(|b| effective_body_span(&b)),
        }
    }
}

/// Prism gives a block/lambda body carrying `rescue`/`else`/`ensure`
/// clauses a keyword-less `BeginNode` spanning the *whole* enclosing
/// construct (`do`/`{` through `end`/`}` included) -- confirmed against
/// `Prism.parse("foo do\n  bar\nrescue => e\n  baz\nend\n")`, whose `body`
/// is a `BeginNode` with no `begin_keyword_loc` and a `location` identical
/// to the `BlockNode`'s own. Whitequark's equivalent `:rescue`/`:ensure`
/// node covers only the clauses themselves, so unwrapped straight through,
/// this shape would make `same_line?(node.loc.begin, node.body)` compare
/// the opening delimiter against *itself* and spuriously report every
/// multi-line rescue-bearing block. This reduces it to the same semantic
/// start: the first present clause's own start (the protected
/// `statements`, else `rescue`, else `else`, else `ensure`).
fn effective_body_span(node: &Node<'_>) -> Span {
    let whole = node.span();
    let Node::BeginNode { .. } = node else { return whole };
    let begin = node.as_begin_node().expect("kind matched");
    if begin.begin_keyword_loc().is_some() {
        return whole;
    }
    let start = begin
        .statements()
        .map(|s| s.as_node().span().start)
        .or_else(|| begin.rescue_clause().map(|r| r.as_node().span().start))
        .or_else(|| begin.else_clause().map(|e| e.as_node().span().start))
        .or_else(|| begin.ensure_clause().map(|e| e.as_node().span().start))
        .unwrap_or(whole.start);
    Span::new(start, whole.end)
}

/// RuboCop-AST's `BlockNode#arguments?` shape check: true unless `node` is
/// Prism's numbered-parameter or `it`-parameter implicit shape.
fn explicit_params(node: &Node<'_>) -> bool {
    !matches!(node, Node::NumberedParametersNode { .. } | Node::ItParametersNode { .. })
}

/// The inner `ParametersNode` actually holding the individual parameter
/// nodes, for either an explicit pipe/paren-delimited `BlockParametersNode`
/// or a bare `ParametersNode` (a parenthesis-less stabby lambda).
fn inner_parameters<'pr>(node: &Node<'pr>) -> Option<ParametersNode<'pr>> {
    match node {
        Node::BlockParametersNode { .. } => {
            node.as_block_parameters_node().and_then(|b| b.parameters())
        }
        Node::ParametersNode { .. } => node.as_parameters_node(),
        _ => None,
    }
}

impl Rule for MultilineBlockLayout {
    const META: RuleMeta = RuleMeta {
        name: "Layout/MultilineBlockLayout",
        department: Department::Layout,
        summary: "Ensures newlines after multiline block do statements.",
        explanation: "\
Checks whether the multiline do end blocks have a newline after the start of
the block. Additionally, it checks whether the block arguments, if any, are
on the same line as the start of the block. Putting block arguments on
separate lines, because the whole line would otherwise be too long, is
accepted.

```ruby
# bad
blah do |i| foo(i)
  bar(i)
end

# bad
blah do
  |i| foo(i)
  bar(i)
end

# good
blah do |i|
  foo(i)
  bar(i)
end

# bad
blah { |i| foo(i)
  bar(i)
}

# good
blah { |i|
  foo(i)
  bar(i)
}
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::LambdaNode],
        config: &[],
        blind_spots: "\
`block_arg_string`'s recursive-parenthesization and trailing-comma-preserving
autocorrection is ported exactly for the shapes RuboCop's own spec covers:
plain, splat, and destructured (`(a, b)`, nested arbitrarily deep) required
parameters, plus a single required parameter followed by a bare trailing
comma. A trailing comma nested inside a destructured group rather than at
the top level (`|(a,)|`) is not specially preserved, matching no known
upstream spec coverage either.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let max_line_length = {
            let enabled = options
                .peer("Layout/LineLength", "Enabled")
                .and_then(OptionValue::as_bool)
                .unwrap_or(true);
            enabled.then(|| {
                options
                    .peer("Layout/LineLength", "Max")
                    .and_then(OptionValue::as_int)
                    .unwrap_or(120)
            })
        };
        Ok(Self { max_line_length })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                if let Some(block) = BlockLike::from_call(&call) {
                    self.check(ctx, &block);
                }
            }
            Node::LambdaNode { .. } => {
                let lambda = node.as_lambda_node().expect("kind matched");
                let block = BlockLike::from_lambda(&lambda);
                self.check(ctx, &block);
            }
            _ => {}
        }
    }
}

impl MultilineBlockLayout {
    /// RuboCop's `on_block`.
    fn check(&self, ctx: &mut Context<'_>, block: &BlockLike<'_>) {
        // RuboCop-AST overrides `BlockNode#single_line?` from the generic
        // whole-expression check to just the opening/closing delimiters
        // (see `BlockLike::closing`'s doc), ignoring a multi-line
        // method-chain receiver entirely.
        if ctx.same_line(block.opening, block.closing) {
            return;
        }
        if let Some(params) = &block.params {
            let params_span = params.span();
            let args_on_beginning_line =
                ctx.line_col(block.opening.start).line == ctx.last_line(params_span);
            if !args_on_beginning_line && !self.line_break_necessary(ctx, block, params) {
                let fix = build_fix(ctx, block, true);
                Self::report(ctx, params_span, ARG_MSG, fix);
            }
        }
        if let Some(body_span) = block.body {
            if ctx.same_line(block.opening, body_span) {
                let fix = build_fix(ctx, block, false);
                Self::report(ctx, body_span, MSG, fix);
            }
        }
    }

    /// RuboCop's `line_break_necessary_in_args?`.
    fn line_break_necessary(
        &self,
        ctx: &Context<'_>,
        block: &BlockLike<'_>,
        params: &Node<'_>,
    ) -> bool {
        let Some(max) = self.max_line_length else { return false };
        i64::from(needed_length_for_args(ctx, block, params)) > max
    }

    fn report(ctx: &mut Context<'_>, span: Span, message: &'static str, fix: Option<Fix>) {
        match fix {
            Some(fix) => ctx.report_with_fix(&Self::META, span, message, fix),
            None => ctx.report(&Self::META, span, message),
        }
    }
}

/// RuboCop's `needed_length_for_args`.
fn needed_length_for_args(ctx: &Context<'_>, block: &BlockLike<'_>, params: &Node<'_>) -> u32 {
    let column = ctx.display_column(block.whole.start);
    let start_line = ctx.line_col(block.whole.start).line;
    let physical_line = ctx.line_span(start_line);
    let first_line_span = Span::new(block.whole.start, physical_line.end);
    let first_line_text = ctx.text(first_line_span);
    // RuboCop's `characters_needed_for_space_and_pipes`.
    let extra_pipes = if first_line_text.last() == Some(&b'|') { 1 } else { 3 };
    column
        + extra_pipes
        + char_len(first_line_text)
        + char_len(render_params(ctx, params).as_bytes())
}

/// RuboCop's `autocorrect`.
fn build_fix(ctx: &Context<'_>, block: &BlockLike<'_>, fix_args: bool) -> Option<Fix> {
    let mut edits = Vec::new();
    let mut expr_before_body = block.opening.start;
    if fix_args {
        let params = block.params.as_ref()?;
        let params_span = params.span();
        let arg_string = render_params(ctx, params);
        let end = ctx.with_surrounding_space(params_span, Side::Right, false, false).end;
        let replace_span = Span::new(block.opening.end, end);
        edits.push(Edit::replace(replace_span, format!(" |{arg_string}|").into_bytes()));
        expr_before_body = params_span.end;
    }
    if let Some(body_span) = block.body {
        if ctx.same_line(Span::empty(expr_before_body), body_span) {
            let indent = ctx.display_column(block.whole.start);
            let text = format!("\n  {}", " ".repeat(indent as usize));
            edits.push(Edit::insert(body_span.start, text.into_bytes()));
        }
    }
    if edits.is_empty() {
        None
    } else {
        Some(Fix { applicability: Applicability::Safe, edits })
    }
}

/// RuboCop's `block_arg_string`, rendering `params`'s parameter list as it
/// would read joined onto a single line -- recursing into a destructured
/// `(a, b)` group (a Prism `MultiTargetNode`) -- plus
/// `include_trailing_comma?`'s special case: a lone required parameter
/// followed by a bare trailing comma (Prism's `ImplicitRestNode`) keeps that
/// comma.
fn render_params(ctx: &Context<'_>, params: &Node<'_>) -> String {
    let Some(parameters) = inner_parameters(params) else { return String::new() };
    let mut parts = Vec::new();
    let mut required_count = 0u32;
    for item in &parameters.requireds() {
        push_item(ctx, &item, &mut parts, &mut required_count);
    }
    for item in &parameters.optionals() {
        push_item(ctx, &item, &mut parts, &mut required_count);
    }
    let mut implicit_rest = false;
    if let Some(rest) = parameters.rest() {
        if matches!(rest, Node::ImplicitRestNode { .. }) {
            implicit_rest = true;
        } else {
            push_item(ctx, &rest, &mut parts, &mut required_count);
        }
    }
    for item in &parameters.posts() {
        push_item(ctx, &item, &mut parts, &mut required_count);
    }
    for item in &parameters.keywords() {
        push_item(ctx, &item, &mut parts, &mut required_count);
    }
    if let Some(kwrest) = parameters.keyword_rest() {
        push_item(ctx, &kwrest, &mut parts, &mut required_count);
    }
    if let Some(blk) = parameters.block() {
        parts.push(text_of(ctx, blk.as_node().span()));
    }
    let mut joined = parts.join(", ");
    if implicit_rest && required_count == 1 {
        joined.push(',');
    }
    joined
}

/// One item of `render_params`/`render_multi_target`'s parameter list:
/// recurses through a nested destructured group, else takes the parameter's
/// own source text verbatim.
fn push_item(
    ctx: &Context<'_>,
    node: &Node<'_>,
    parts: &mut Vec<String>,
    required_count: &mut u32,
) {
    if let Node::MultiTargetNode { .. } = node {
        let mt = node.as_multi_target_node().expect("kind matched");
        parts.push(format!("({})", render_multi_target(ctx, &mt, required_count)));
        return;
    }
    if matches!(node, Node::RequiredParameterNode { .. }) {
        *required_count += 1;
    }
    parts.push(text_of(ctx, node.span()));
}

/// RuboCop's `block_arg_string` recursing into an `mlhs`-shaped (Prism
/// `MultiTargetNode`) destructured group: every left-of-splat target, the
/// splat itself (if named), then every right-of-splat target, comma-joined.
fn render_multi_target(
    ctx: &Context<'_>,
    mt: &MultiTargetNode<'_>,
    required_count: &mut u32,
) -> String {
    let mut parts = Vec::new();
    for item in &mt.lefts() {
        push_item(ctx, &item, &mut parts, required_count);
    }
    if let Some(rest) = mt.rest() {
        if !matches!(rest, Node::ImplicitRestNode { .. }) {
            push_item(ctx, &rest, &mut parts, required_count);
        }
    }
    for item in &mt.rights() {
        push_item(ctx, &item, &mut parts, required_count);
    }
    parts.join(", ")
}

fn text_of(ctx: &Context<'_>, span: Span) -> String {
    String::from_utf8_lossy(ctx.text(span)).into_owned()
}
