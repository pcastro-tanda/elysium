//! `Layout/FirstArrayElementLineBreak`, ported from RuboCop's
//! `lib/rubocop/cop/layout/first_array_element_line_break.rb` plus the
//! `FirstElementLineBreak` mixin it includes.
//!
//! RuboCop's `FirstElementLineBreak#check_children_line_break` is always
//! called here with its default `start: node` (the array literal itself),
//! so `start.first_line` is just the array's own first line.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::{is_ruby_whitespace, Span};

/// RuboCop's `MSG`.
const MSG: &str = "Add a line break before the first element of a multi-line array.";

/// Checks for a line break before the first element in a multi-line array.
#[derive(Debug, Clone)]
pub struct FirstArrayElementLineBreak {
    allow_implicit_array_literals: bool,
    allow_multiline_final_element: bool,
}

impl Rule for FirstArrayElementLineBreak {
    const META: RuleMeta = RuleMeta {
        name: "Layout/FirstArrayElementLineBreak",
        department: Department::Layout,
        summary: "Checks for a line break before the first element in a multi-line array.",
        explanation: "\
```ruby
# bad
[ :a,
  :b]

# good
[
  :a,
  :b]

# good
[:a, :b]
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ArrayNode],
        config: &[
            ConfigOption {
                name: "AllowImplicitArrayLiterals",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether an implicit (bracket-less) array literal, e.g. the right-hand \
                      side of a multiple assignment, is exempt from this check.",
            },
            ConfigOption {
                name: "AllowMultilineFinalElement",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether the last element of the array is allowed to start a new, \
                      multi-line element without triggering this cop.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_implicit_array_literals: options.bool("AllowImplicitArrayLiterals"),
            allow_multiline_final_element: options.bool("AllowMultilineFinalElement"),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(array) = node.as_array_node() else { return };
        let node_span = array.as_node().span();

        if array.opening_loc().is_none() && !assignment_on_same_line(ctx, node_span) {
            return;
        }
        if self.allow_implicit_array_literals && array.opening_loc().is_none() {
            return;
        }

        let children: Vec<Span> = array.elements().iter().map(|n| n.span()).collect();
        check_children_line_break(ctx, node_span, &children, self.allow_multiline_final_element);
    }
}

/// RuboCop's `assignment_on_same_line?`: does `node`'s own line, up to
/// `node`'s own column, end (after trimming trailing whitespace) in `=`?
fn assignment_on_same_line(ctx: &Context<'_>, node_span: Span) -> bool {
    let line = ctx.line_col(node_span.start).line;
    let line_start = ctx.line_span(line).start;
    let col = usize::try_from(node_span.start - line_start).unwrap_or(usize::MAX);
    trimmed_prefix_ends_with(ctx.line_text(line), col, b'=')
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
    ctx.report_with_fix(&FirstArrayElementLineBreak::META, min, MSG, fix);
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
