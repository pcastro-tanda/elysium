//! `Style/MultilineMethodSignature`, ported from RuboCop's
//! `lib/rubocop/cop/style/multiline_method_signature.rb`.
//!
//! whitequark's `node.arguments` (used for `.last_line`, the line the
//! *closing paren* sits on) is the args-wrapper node, whose own
//! `loc.expression` spans from `(` to `)`; a parameter *node*'s own span
//! never includes the parens. Prism's `ParametersNode` has no such
//! paren-inclusive span of its own, so `DefNode`'s own `lparen_loc`/
//! `rparen_loc` are used directly in its place throughout (including as
//! `begin_of_arguments`/`arguments.loc.end`).
//!
//! `node.arguments` (used for `.map(&:source).join(', ')`, the individual
//! parameter nodes in source order) is Prism's `ParametersNode` split by
//! category (`requireds`/`optionals`/`rest`/`posts`/`keywords`/
//! `keyword_rest`/`block`) rather than one ordered list; [`collect_params`]
//! re-derives the original order by sorting on each parameter's own byte
//! offset.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::ParametersNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

const MSG: &str = "Avoid multi-line method signatures.";

/// Checks for method signatures that span multiple lines.
#[derive(Debug, Clone)]
pub struct MultilineMethodSignature {
    /// `Layout/LineLength`'s own `Max`, if that cop is enabled; `None` if
    /// disabled, matching upstream's `max_line_length` returning `nil`
    /// (the correction-exceeds-max-length guard never trips).
    max_line_length: Option<i64>,
}

impl Rule for MultilineMethodSignature {
    const META: RuleMeta = RuleMeta {
        name: "Style/MultilineMethodSignature",
        department: Department::Style,
        summary: "Avoid multi-line method signatures.",
        explanation: "\
```ruby
# good

def foo(bar, baz)
end

# bad

def foo(bar,
        baz)
end
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let enabled = options
            .peer("Layout/LineLength", "Enabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(true);
        let max_line_length = enabled.then(|| {
            options.peer("Layout/LineLength", "Max").and_then(OptionValue::as_int).unwrap_or(120)
        });
        Ok(Self { max_line_length })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        let Some(params) = def.parameters() else { return };
        if !params_nonempty(&params) {
            return;
        }
        let Some(lparen) = def.lparen_loc().map(|l| l.span()) else { return };
        let Some(rparen) = def.rparen_loc().map(|l| l.span()) else { return };

        let opening_line = ctx.line_col(def.as_node().span().start).line;
        let closing_line = ctx.line_col(rparen.start).line;
        if opening_line == closing_line {
            return;
        }
        if self.correction_exceeds_max_line_length(ctx, &def, rparen) {
            return;
        }

        let mut params_nodes = collect_params(&params);
        params_nodes.sort_by_key(|n| n.span().start);
        let Some(first) = params_nodes.first().copied() else { return };
        let last = params_nodes.last().copied().unwrap_or(first);

        let fix = build_fix(ctx, &def, &params_nodes, first, last, lparen, rparen, opening_line);
        ctx.report_with_fix(&Self::META, def.as_node().span(), MSG, fix);
    }
}

impl MultilineMethodSignature {
    /// `correction_exceeds_max_line_length?`.
    fn correction_exceeds_max_line_length(
        &self,
        ctx: &Context<'_>,
        def: &ruby_ast::node::DefNode<'_>,
        rparen: Span,
    ) -> bool {
        let Some(max) = self.max_line_length else { return false };
        let def_start = def.as_node().span().start;
        let line = ctx.line_col(def_start).line;
        let line_text = ctx.line_text(line);
        let indentation = line_text.len() - line_text.trim_ascii_start().len();

        let signature = ctx.text(Span::new(def_start, rparen.end));
        let collapsed = collapse_whitespace(signature);

        i64::try_from(indentation + collapsed.len()).unwrap_or(i64::MAX) > max
    }
}

/// `ParameterizedNode#arguments?`: does this parameter list have at least
/// one actual parameter (an empty `()` does not count).
fn params_nonempty(params: &ParametersNode<'_>) -> bool {
    params.requireds().iter().count() > 0
        || params.optionals().iter().count() > 0
        || params.rest().is_some()
        || params.posts().iter().count() > 0
        || params.keywords().iter().count() > 0
        || params.keyword_rest().is_some()
        || params.block().is_some()
}

/// Every individual parameter node, in whatever order Prism happened to
/// categorize them -- sorted by the caller into source order.
fn collect_params<'pr>(params: &ParametersNode<'pr>) -> Vec<Node<'pr>> {
    let mut out = Vec::new();
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

/// Collapses every run of ASCII whitespace to a single space.
fn collapse_whitespace(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut in_ws = false;
    for &b in bytes {
        if b.is_ascii_whitespace() {
            if !in_ws {
                out.push(b' ');
                in_ws = true;
            }
        } else {
            out.push(b);
            in_ws = false;
        }
    }
    out
}

/// `autocorrect`.
#[allow(clippy::too_many_arguments)]
fn build_fix(
    ctx: &Context<'_>,
    def: &ruby_ast::node::DefNode<'_>,
    params_nodes: &[Node<'_>],
    first: Node<'_>,
    last: Node<'_>,
    lparen: Span,
    rparen: Span,
    opening_line: u32,
) -> Fix {
    let mut joined_arguments: Vec<u8> =
        params_nodes.iter().map(|n| ctx.text(n.span())).collect::<Vec<_>>().join(&b", "[..]);

    let mut edits = Vec::new();

    let closing_line = ctx.line_col(rparen.start).line;
    let last_line_stripped = ctx.line_text(closing_line).trim_ascii();
    if last_line_stripped.starts_with(b")") {
        joined_arguments.extend_from_slice(last_line_stripped);
        edits.push(Edit::delete(ctx.whole_lines(rparen)));
    }

    let core_range = Span::new(first.span().start, last.span().end);
    let arguments_range = ctx.with_surrounding_space(core_range, Side::Left, true, false);

    if ctx.line_col(arguments_range.start).line != opening_line {
        let prefix_range = Span::new(def.def_keyword_loc().span().end, lparen.start);
        let stripped = ctx.text(prefix_range).trim_ascii();
        let mut replacement = vec![b' '];
        replacement.extend_from_slice(stripped);
        edits.push(Edit::replace(prefix_range, replacement));
    }

    edits.push(Edit::delete(arguments_range));
    edits.push(Edit::insert(lparen.end, joined_arguments));

    Fix { applicability: Applicability::Safe, edits }
}
