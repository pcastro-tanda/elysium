//! `Layout/ArrayAlignment`, ported from RuboCop's
//! `lib/rubocop/cop/layout/array_alignment.rb` plus the `Alignment` mixin
//! (`lib/rubocop/cop/mixin/alignment.rb`) and `AlignmentCorrector`
//! (`lib/rubocop/cop/correctors/alignment_corrector.rb`) it uses for
//! autocorrection -- shared with `Layout/ParameterAlignment` via
//! [`super::alignment_mixin`].
//!
//! RuboCop's `node.parent&.masgn_type?` guard (skip the implicit array a
//! multiple assignment's right-hand side parses as, e.g. `a, b = 1, 2`) maps
//! onto Prism's [`NodeKind::MultiWriteNode`]: unlike whitequark, which elides
//! brackets from *every* implicit array (both a masgn's RHS and a plain
//! `var = first, second`), Prism gives both the very same bracket-less
//! [`NodeKind::ArrayNode`], distinguished only by which kind of node their
//! parent is. `bracketed?` (`square_brackets? || percent_literal?`) is simply
//! whether Prism's own [`ruby_ast::node::ArrayNode::opening_loc`] is present
//! -- both a `[...]` literal and a `%w[...]`/`%i[...]` literal set it, and a
//! bracket-less array (masgn RHS or `var = a, b`) never does.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::ArrayNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::alignment_mixin::{check_alignment, indentation_of_line};

/// RuboCop's `ALIGN_ELEMENTS_MSG`.
const ALIGN_ELEMENTS_MSG: &str =
    "Align the elements of an array literal if they span more than one line.";
/// RuboCop's `FIXED_INDENT_MSG`.
const FIXED_INDENT_MSG: &str = "Use one level of indentation for elements following the \
                                 first line of a multi-line array.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    WithFirstElement,
    WithFixedIndentation,
}

/// Checks that the elements of a multi-line array literal are aligned,
/// ported from RuboCop's `ArrayAlignment` cop plus its `Alignment` mixin
/// and `AlignmentCorrector`.
#[derive(Debug, Clone)]
pub struct ArrayAlignment {
    style: Style,
    indentation_width: i64,
    /// RuboCop's `@current_offenses`, scoped to this rule's own offenses in
    /// the current file (see [`super::alignment_mixin::check_alignment`]).
    reported: Vec<Span>,
}

impl ArrayAlignment {
    /// RuboCop's `message`.
    const fn message(&self) -> &'static str {
        match self.style {
            Style::WithFixedIndentation => FIXED_INDENT_MSG,
            Style::WithFirstElement => ALIGN_ELEMENTS_MSG,
        }
    }

    /// RuboCop's `base_column`.
    fn base_column(&self, array: &ArrayNode<'_>, first: &Node<'_>, ctx: &Context<'_>) -> i64 {
        if self.style == Style::WithFixedIndentation {
            let line = target_method_lineno(array, ctx);
            i64::from(indentation_of_line(ctx, line)) + self.indentation_width
        } else {
            i64::from(ctx.display_column(first.span().start))
        }
    }
}

impl Rule for ArrayAlignment {
    const META: RuleMeta = RuleMeta {
        name: "Layout/ArrayAlignment",
        department: Department::Layout,
        summary: "Align the elements of an array literal if they span more than one line.",
        explanation: "\
Checks that the elements of a multi-line array literal are aligned.

```ruby
# EnforcedStyle: with_first_element (default)

# good

array = [1, 2, 3,
         4, 5, 6]
array = ['run',
         'forrest',
         'run']

# bad

array = [1, 2, 3,
  4, 5, 6]
array = ['run',
     'forrest',
     'run']
```

```ruby
# EnforcedStyle: with_fixed_indentation

# good

array = [1, 2, 3,
  4, 5, 6]

# bad

array = [1, 2, 3,
         4, 5, 6]
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ArrayNode],
        config: &[
            linter::ConfigOption {
                name: "EnforcedStyle",
                default: linter::ConfigDefault::Str("with_first_element"),
                allowed: &["with_first_element", "with_fixed_indentation"],
                doc: "Aligns following lines with the first element \
                      (`with_first_element`) or one indentation level past \
                      the line the array starts on (`with_fixed_indentation`).",
            },
            linter::ConfigOption {
                name: "IndentationWidth",
                default: linter::ConfigDefault::Nil,
                allowed: &[],
                doc: "Overrides `Layout/IndentationWidth`'s configured width \
                      for `with_fixed_indentation`'s base column; falls back \
                      to it, else 2.",
            },
        ],
        blind_spots: "\
Autocorrection's taboo-range protection (RuboCop's `AlignmentCorrector`
`inside_string_ranges`) only covers heredoc bodies; the interior of an
ordinary multi-line quoted string or `%`-literal that itself begins a
physical line inside a misaligned element is not separately protected.
The block-comment guard is a per-line `=begin` text match rather than
resolving actual `EmbDoc` comment nodes, matching this crate's other
`Alignment`-based cops.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "with_fixed_indentation" => Style::WithFixedIndentation,
            _ => Style::WithFirstElement,
        };
        let indentation_width = options
            .get("IndentationWidth")
            .and_then(linter::OptionValue::as_int)
            .or_else(|| {
                options
                    .peer("Layout/IndentationWidth", "Width")
                    .and_then(linter::OptionValue::as_int)
            })
            .unwrap_or(2);
        Ok(Self { style, indentation_width, reported: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.reported.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Node::ArrayNode { .. } = node else { return };
        let array = node.as_array_node().expect("kind matched");
        let elements = array.elements();
        if elements.len() < 2 {
            return;
        }
        if ctx.parent().is_some_and(|parent| parent.kind == NodeKind::MultiWriteNode) {
            return;
        }
        let items: Vec<Node<'_>> = elements.iter().collect();
        let base_column = self.base_column(&array, &items[0], ctx);
        let message = self.message();
        check_alignment(ctx, &Self::META, &items, base_column, message, &mut self.reported);
    }
}

/// RuboCop's `target_method_lineno`: the line of the array's own opening
/// bracket for a bracketed array (`bracketed?`), or its parent's line for a
/// bracket-less one.
fn target_method_lineno(array: &ArrayNode<'_>, ctx: &Context<'_>) -> u32 {
    if array.opening_loc().is_some() {
        ctx.line_col(array.location().span().start).line
    } else {
        ctx.parent().map_or_else(
            || ctx.line_col(array.location().span().start).line,
            |parent| ctx.line_col(parent.span.start).line,
        )
    }
}
