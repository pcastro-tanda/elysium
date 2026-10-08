//! `Layout/FirstHashElementLineBreak`, ported from RuboCop's
//! `lib/rubocop/cop/layout/first_hash_element_line_break.rb` plus the
//! `FirstElementLineBreak` mixin it includes.
//!
//! Prism only produces a `HashNode` for a braced hash literal (`node.loc.begin`
//! is always present); a braceless keyword-argument hash is a distinct
//! `KeywordHashNode`, which `Layout/FirstMethodArgumentLineBreak` handles
//! instead, so `on_hash`'s `return unless node.loc.begin` guard is dead code
//! here -- every `HashNode` reaching [`FirstHashElementLineBreak::enter`]
//! already qualifies.
//!
//! RuboCop's `FirstElementLineBreak#check_children_line_break` is always
//! called here with its default `start: node` (the hash literal itself), so
//! `start.first_line` is just the hash's own first line.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Add a line break before the first element of a multi-line hash.";

/// Checks for a line break before the first element in a multi-line hash.
#[derive(Debug, Clone)]
pub struct FirstHashElementLineBreak {
    allow_multiline_final_element: bool,
}

impl Rule for FirstHashElementLineBreak {
    const META: RuleMeta = RuleMeta {
        name: "Layout/FirstHashElementLineBreak",
        department: Department::Layout,
        summary: "Checks for a line break before the first element in a multi-line hash.",
        explanation: "\
```ruby
# bad
{ a: 1,
  b: 2}

# good
{
  a: 1,
  b: 2 }
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::HashNode],
        config: &[ConfigOption {
            name: "AllowMultilineFinalElement",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Whether the last element of the hash is allowed to start a new, \
                  multi-line element without triggering this cop.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_multiline_final_element: options.bool("AllowMultilineFinalElement") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(hash) = node.as_hash_node() else { return };
        let node_span = hash.as_node().span();
        let children: Vec<Span> = hash.elements().iter().map(|n| n.span()).collect();
        check_children_line_break(ctx, node_span, &children, self.allow_multiline_final_element);
    }
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
    ctx.report_with_fix(&FirstHashElementLineBreak::META, min, MSG, fix);
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
