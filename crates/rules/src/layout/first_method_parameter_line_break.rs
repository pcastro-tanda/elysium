//! `Layout/FirstMethodParameterLineBreak`, ported from RuboCop's
//! `lib/rubocop/cop/layout/first_method_parameter_line_break.rb` plus the
//! `FirstElementLineBreak` mixin it includes.
//!
//! Whitequark's `args` node already lists every parameter kind in a single
//! textually-ordered array (RuboCop's `node.arguments`); Prism's
//! `ParametersNode` instead splits them into separate `requireds`/
//! `optionals`/`rest`/`posts`/`keywords`/`keyword_rest`/`block` fields, so
//! [`FirstMethodParameterLineBreak::enter`] gathers all of them and sorts by
//! source position to reconstruct that same textual order (both for
//! `method_uses_parens?`'s "first parameter" and for `first_by_line`'s
//! tie-break-by-declaration-order on parameters sharing a first line).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::{is_ruby_whitespace, Span};

/// RuboCop's `MSG`.
const MSG: &str =
    "Add a line break before the first parameter of a multi-line method parameter list.";

/// Checks for a line break before the first parameter in a multi-line method parameter definition.
#[derive(Debug, Clone)]
pub struct FirstMethodParameterLineBreak {
    allow_multiline_final_element: bool,
}

impl Rule for FirstMethodParameterLineBreak {
    const META: RuleMeta = RuleMeta {
        name: "Layout/FirstMethodParameterLineBreak",
        department: Department::Layout,
        summary: "Checks for a line break before the first parameter in a multi-line method \
                   parameter definition.",
        explanation: "\
```ruby
# bad
def method(foo, bar,
    baz)
  do_something
end

# good
def method(
    foo, bar,
    baz)
  do_something
end

# ignored
def method foo,
    bar
  do_something
end
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[ConfigOption {
            name: "AllowMultilineFinalElement",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Whether the last parameter is allowed to start a new, multi-line \
                  element without triggering this cop.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_multiline_final_element: options.bool("AllowMultilineFinalElement") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        let Some(params) = def.parameters() else { return };

        let mut children: Vec<Span> = Vec::new();
        children.extend(params.requireds().iter().map(|n| n.span()));
        children.extend(params.optionals().iter().map(|n| n.span()));
        if let Some(rest) = params.rest() {
            children.push(rest.span());
        }
        children.extend(params.posts().iter().map(|n| n.span()));
        children.extend(params.keywords().iter().map(|n| n.span()));
        if let Some(kwrest) = params.keyword_rest() {
            children.push(kwrest.span());
        }
        if let Some(block) = params.block() {
            children.push(block.as_node().span());
        }
        children.sort_by_key(|s| s.start);

        check_method_line_break(
            ctx,
            def.as_node().span(),
            &children,
            self.allow_multiline_final_element,
        );
    }
}

/// RuboCop's `FirstElementLineBreak#check_method_line_break`.
fn check_method_line_break(
    ctx: &mut Context<'_>,
    node_span: Span,
    children: &[Span],
    ignore_last: bool,
) {
    if children.is_empty() {
        return;
    }
    if !method_uses_parens(ctx, node_span, children[0]) {
        return;
    }
    check_children_line_break(ctx, node_span, children, ignore_last);
}

/// RuboCop's `FirstElementLineBreak#method_uses_parens?`: slices `node`'s own
/// first line up to `limit`'s own column (on `limit`'s own line, which may
/// differ from `node`'s), and checks the trimmed prefix ends in `(`.
fn method_uses_parens(ctx: &Context<'_>, node_span: Span, limit: Span) -> bool {
    let node_line = ctx.line_col(node_span.start).line;
    let limit_line = ctx.line_col(limit.start).line;
    let limit_line_start = ctx.line_span(limit_line).start;
    let col = usize::try_from(limit.start - limit_line_start).unwrap_or(usize::MAX);
    trimmed_prefix_ends_with(ctx.line_text(node_line), col, b'(')
}

/// `prefix[0...col]`, trailing-whitespace-trimmed, ends in `byte`.
fn trimmed_prefix_ends_with(line: &[u8], col: usize, byte: u8) -> bool {
    let col = col.min(line.len());
    let mut i = col;
    while i > 0 && is_ruby_whitespace(line[i - 1]) {
        i -= 1;
    }
    i > 0 && line[i - 1] == byte
}

/// RuboCop's `FirstElementLineBreak#check_children_line_break`, with the
/// default `start: node`.
fn check_children_line_break(
    ctx: &mut Context<'_>,
    node_span: Span,
    children: &[Span],
    ignore_last: bool,
) {
    if children.is_empty() {
        return;
    }

    let line = ctx.line_col(node_span.start).line;

    let min = first_by_line(ctx, children);
    if line != ctx.line_col(min.start).line {
        return;
    }

    let max_line = children
        .iter()
        .map(|&c| if ignore_last { ctx.line_col(c.start).line } else { ctx.last_line(c) })
        .max()
        .expect("non-empty");
    if line == max_line {
        return;
    }

    let fix = Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::insert(min.start, b"\n".to_vec())],
    };
    ctx.report_with_fix(&FirstMethodParameterLineBreak::META, min, MSG, fix);
}

/// RuboCop's `first_by_line`: `nodes.min_by(&:first_line)`, which keeps the
/// first element reached on ties.
fn first_by_line(ctx: &Context<'_>, children: &[Span]) -> Span {
    let mut min = children[0];
    let mut min_line = ctx.line_col(min.start).line;
    for &c in &children[1..] {
        let l = ctx.line_col(c.start).line;
        if l < min_line {
            min = c;
            min_line = l;
        }
    }
    min
}
