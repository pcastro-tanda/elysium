//! `Layout/SpaceAroundBlockParameters`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_around_block_parameters.rb` plus the
//! `ConfigurableEnforcedStyle` and `RangeHelp` mixins it uses.
//!
//! A Prism block/lambda parameter list is always a single
//! [`NodeKind::BlockParametersNode`] (`BlockNode`/`LambdaNode#parameters`),
//! whether written with pipes (`|x, y|`) or parens (`->(x, y)`); its
//! `opening_loc`/`closing_loc` are `None` for a paren/pipe-less lambda
//! (`->x { }`) and `Some` otherwise, mirroring RuboCop-AST's
//! `arguments.loc.begin`/`.end` pair used by `pipes?`. A numbered
//! (`_1`)/`it`-parameter block has no `BlockParametersNode` at all
//! (`NumberedParametersNode`/`ItParametersNode` instead), which this cop
//! deliberately never handles upstream (see its `on_block` comment
//! disabling `InternalAffairs/NumblockHandler`/`ItblockHandler`).
//!
//! RuboCop's `add_offense` silently drops a second offense at a byte range
//! already reported by this same cop within the file
//! (`Base#add_offense`'s `current_offense_locations.add?`); one real case
//! here is the generic per-parameter "Extra space before" check
//! (`check_arg`) coinciding exactly with the pipe-adjacent "Extra space
//! before first"/"Space before first" check's range for the first
//! parameter. [`SpaceAroundBlockParameters::reported`] reproduces that
//! per-file span dedup.

use std::collections::HashSet;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::ParametersNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// RuboCop's `EnforcedStyleInsidePipes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    NoSpace,
    Space,
}

/// Checks the spacing inside and after block parameters pipes.
#[derive(Debug, Clone)]
pub struct SpaceAroundBlockParameters {
    style: Style,
    /// Precomputed `conflicting_space_inside_parens_style?` result for a
    /// paren-delimited (lambda) parameter list: whether the peer
    /// `Layout/SpaceInsideParens`'s enabled `EnforcedStyle` contradicts
    /// `style`. Never true for a pipe-delimited list (RuboCop's own
    /// `arguments.loc.begin.source == '('` guard).
    parens_conflict: bool,
    /// Byte spans already reported by this rule in the current file,
    /// mirroring RuboCop's `current_offense_locations` per-cop dedup.
    reported: HashSet<Span>,
}

impl Rule for SpaceAroundBlockParameters {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceAroundBlockParameters",
        department: Department::Layout,
        summary: "Checks the spacing inside and after block parameters pipes.",
        explanation: "\
Checks the spacing inside and after block parameters pipes. Line breaks
inside parameter pipes are checked by `Layout/MultilineBlockLayout` and
not by this cop. Spaces inside a lambda's parameter parentheses are left
to `Layout/SpaceInsideParens` when that cop enforces a conflicting style.

```ruby
# EnforcedStyleInsidePipes: no_space (default)

# bad
{}.each { | x,  y |puts x }
->( x,  y ) { puts x }

# good
{}.each { |x, y| puts x }
->(x, y) { puts x }
```

```ruby
# EnforcedStyleInsidePipes: space

# bad
{}.each { |x,  y| puts x }
->(x,  y) { puts x }

# good
{}.each { | x, y | puts x }
->( x, y ) { puts x }
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::BlockNode, NodeKind::LambdaNode],
        config: &[ConfigOption {
            name: "EnforcedStyleInsidePipes",
            default: ConfigDefault::Str("no_space"),
            allowed: &["space", "no_space"],
            doc: "Whether a single space is required (`space`) or forbidden \
                  (`no_space`) right inside the parameter pipes/parens.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyleInsidePipes")? {
            "space" => Style::Space,
            _ => Style::NoSpace,
        };
        let peer_enabled = options
            .peer("Layout/SpaceInsideParens", "Enabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(true);
        let parens_conflict = peer_enabled
            && match options
                .peer("Layout/SpaceInsideParens", "EnforcedStyle")
                .and_then(OptionValue::as_str)
                .unwrap_or("no_space")
            {
                "no_space" => style == Style::Space,
                "space" | "compact" => style == Style::NoSpace,
                _ => false,
            };
        Ok(Self { style, parens_conflict, reported: HashSet::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.reported.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (parameters, body) = match node {
            Node::BlockNode { .. } => {
                let block = node.as_block_node().expect("kind matched");
                (block.parameters(), block.body())
            }
            Node::LambdaNode { .. } => {
                let lambda = node.as_lambda_node().expect("kind matched");
                (lambda.parameters(), lambda.body())
            }
            _ => return,
        };
        let Some(parameters) = parameters else { return };
        let Some(block_params) = parameters.as_block_parameters_node() else { return };
        let Some(opening) = block_params.opening_loc() else { return };
        let Some(closing) = block_params.closing_loc() else { return };
        let opening_span = opening.span();
        let closing_span = closing.span();
        let Some(inner) = block_params.parameters() else { return };
        let args = collect_params(&inner);
        if args.is_empty() {
            return;
        }
        let whole_span = block_params.location().span();
        let is_parens = ctx.text(opening_span) == b"(";

        if !(is_parens && self.parens_conflict) {
            self.check_inside_pipes(ctx, &args, opening_span, closing_span, whole_span);
        }
        if let Some(body) = body {
            self.check_space(
                ctx,
                closing_span.end,
                body.span().start,
                closing_span,
                "after closing `|`",
                false,
            );
        }
        self.check_each_arg(ctx, &args);
    }
}

impl SpaceAroundBlockParameters {
    fn check_inside_pipes(
        &mut self,
        ctx: &mut Context<'_>,
        args: &[Node<'_>],
        opening_span: Span,
        closing_span: Span,
        whole_span: Span,
    ) {
        match self.style {
            Style::NoSpace => {
                self.check_no_space_style_inside_pipes(
                    ctx,
                    args,
                    opening_span,
                    closing_span,
                    whole_span,
                );
            }
            Style::Space => {
                self.check_space_style_inside_pipes(
                    ctx,
                    args,
                    opening_span,
                    closing_span,
                    whole_span,
                );
            }
        }
    }

    fn check_no_space_style_inside_pipes(
        &mut self,
        ctx: &mut Context<'_>,
        args: &[Node<'_>],
        opening_span: Span,
        closing_span: Span,
        whole_span: Span,
    ) {
        let first = args.first().expect("non-empty").span();
        let last = args.last().expect("non-empty").span();
        self.check_no_space(ctx, opening_span.end, first.start, "Space before first");
        let last_end = last_end_pos_inside_pipes(ctx, whole_span, last);
        self.check_no_space(ctx, last_end, closing_span.start, "Space after last");
    }

    fn check_space_style_inside_pipes(
        &mut self,
        ctx: &mut Context<'_>,
        args: &[Node<'_>],
        opening_span: Span,
        closing_span: Span,
        whole_span: Span,
    ) {
        let first = args.first().expect("non-empty").span();
        self.check_space(
            ctx,
            opening_span.end,
            first.start,
            first,
            "before first block parameter",
            true,
        );
        self.check_no_space(
            ctx,
            opening_span.end,
            first.start.saturating_sub(1),
            "Extra space before first",
        );

        let last = args.last().expect("non-empty").span();
        let last_end = last_end_pos_inside_pipes(ctx, whole_span, last);
        self.check_space(
            ctx,
            last_end,
            closing_span.start,
            last,
            "after last block parameter",
            false,
        );
        self.check_no_space(ctx, last_end + 1, closing_span.start, "Extra space after last");
    }

    fn check_each_arg(&mut self, ctx: &mut Context<'_>, args: &[Node<'_>]) {
        for arg in args {
            self.check_arg(ctx, arg);
        }
    }

    /// RuboCop's `check_arg`: recurses into a destructured (`(a, b)`)
    /// parameter's own children first, then always checks for extra space
    /// immediately preceding the node's own span.
    fn check_arg(&mut self, ctx: &mut Context<'_>, arg: &Node<'_>) {
        if let Some(mlhs) = arg.as_multi_target_node() {
            for left in &mlhs.lefts() {
                self.check_arg(ctx, &left);
            }
            if let Some(rest) = mlhs.rest() {
                if rest.as_implicit_rest_node().is_none() {
                    self.check_arg(ctx, &rest);
                }
            }
            for right in &mlhs.rights() {
                self.check_arg(ctx, &right);
            }
        }
        let expr = arg.span();
        let expanded = ctx.with_surrounding_space(expr, Side::Left, true, false);
        self.check_no_space(
            ctx,
            expanded.start,
            expr.start.saturating_sub(1),
            "Extra space before",
        );
    }

    /// RuboCop's `check_space`: reports a missing single space as an
    /// insertion, either right before `target` (`insert_before: true`) or
    /// right after it.
    fn check_space(
        &mut self,
        ctx: &mut Context<'_>,
        space_begin_pos: u32,
        space_end_pos: u32,
        target: Span,
        msg: &str,
        insert_before: bool,
    ) {
        if space_begin_pos != space_end_pos {
            return;
        }
        if !self.reported.insert(target) {
            return;
        }
        let message = format!("Space {msg} missing.");
        let edit = if insert_before {
            Edit::insert(target.start, b" ".as_slice())
        } else {
            Edit::insert(target.end, b" ".as_slice())
        };
        ctx.report_with_fix(
            &Self::META,
            target,
            message,
            Fix { applicability: Applicability::Safe, edits: vec![edit] },
        );
    }

    /// RuboCop's `check_no_space`: reports a run of whitespace between two
    /// positions as extra, removing all but the last character reserved by
    /// the caller (or all of it, when the caller passed no reserved
    /// character).
    fn check_no_space(
        &mut self,
        ctx: &mut Context<'_>,
        space_begin_pos: u32,
        space_end_pos: u32,
        msg: &str,
    ) {
        if space_begin_pos >= space_end_pos {
            return;
        }
        let span = Span::new(space_begin_pos, space_end_pos);
        if ctx.text(span).contains(&b'\n') {
            return;
        }
        if !self.reported.insert(span) {
            return;
        }
        let message = format!("{msg} block parameter detected.");
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] },
        );
    }
}

/// RuboCop-AST's `BlockNode#arguments`/`LambdaNode#arguments`: every
/// parameter node in source order (Ruby's fixed grammar order --
/// required, optional, rest, post, keyword, keyword-rest, block -- matches
/// textual order), with a bare trailing-comma `ImplicitRestNode` dropped
/// (whitequark's `args` node has no child for it either; the trailing
/// comma itself is instead found by [`last_end_pos_inside_pipes`]'s
/// textual scan).
fn collect_params<'pr>(params: &ParametersNode<'pr>) -> Vec<Node<'pr>> {
    let mut out = Vec::new();
    for n in &params.requireds() {
        out.push(n);
    }
    for n in &params.optionals() {
        out.push(n);
    }
    if let Some(rest) = params.rest() {
        if rest.as_implicit_rest_node().is_none() {
            out.push(rest);
        }
    }
    for n in &params.posts() {
        out.push(n);
    }
    for n in &params.keywords() {
        out.push(n);
    }
    if let Some(kwrest) = params.keyword_rest() {
        out.push(kwrest);
    }
    if let Some(block) = params.block() {
        out.push(block.as_node());
    }
    out
}

/// RuboCop's `last_end_pos_inside_pipes`: the position right after a
/// trailing comma following the last parameter, when the source between
/// the last parameter's end and the closing pipe/paren contains one (a
/// bare trailing comma, `|x, |`, parses with no node for the comma
/// itself); otherwise the last parameter's own end position.
fn last_end_pos_inside_pipes(ctx: &Context<'_>, whole_span: Span, last: Span) -> u32 {
    let pos = last.end;
    let scan = ctx.text(Span::new(pos, whole_span.end));
    match scan.iter().position(|&b| b == b',') {
        Some(idx) => pos + u32::try_from(idx).unwrap_or(0) + 1,
        None => pos,
    }
}
