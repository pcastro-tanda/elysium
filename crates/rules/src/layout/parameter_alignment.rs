//! `Layout/ParameterAlignment`, ported from RuboCop's
//! `lib/rubocop/cop/layout/parameter_alignment.rb` plus the `Alignment`
//! mixin (`lib/rubocop/cop/mixin/alignment.rb`) and `AlignmentCorrector`
//! (`lib/rubocop/cop/correctors/alignment_corrector.rb`) it uses for
//! autocorrection -- shared with `Layout/ArrayAlignment` via
//! [`super::alignment_mixin`].
//!
//! RuboCop-AST's `DefNode#arguments` is simply `children[-2]`: the whole
//! `args` node, iterated over via `Enumerable` -- its children are already
//! every parameter in one list, in source order, regardless of kind
//! (required/optional/rest/post/keyword/kwrest/block). Prism instead splits
//! a [`ruby_ast::node::ParametersNode`] into separate typed lists
//! (`requireds`, `optionals`, `rest`, `posts`, `keywords`, `keyword_rest`,
//! `block`), so [`parameter_list`] concatenates them and sorts by start
//! offset to recover that single source-order list. `on_defs` needs no
//! separate handling: Prism gives both an instance method (`def foo`) and a
//! singleton method (`def self.foo`) the same [`NodeKind::DefNode`].

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::alignment_mixin::{check_alignment, indentation_of_line};

/// RuboCop's `ALIGN_PARAMS_MSG`.
const ALIGN_PARAMS_MSG: &str =
    "Align the parameters of a method definition if they span more than one line.";
/// RuboCop's `FIXED_INDENT_MSG`.
const FIXED_INDENT_MSG: &str = "Use one level of indentation for parameters following the \
                                 first line of a multi-line method definition.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    WithFirstParameter,
    WithFixedIndentation,
}

/// Checks that the parameters of a multi-line method definition are
/// aligned, ported from RuboCop's `ParameterAlignment` cop plus its
/// `Alignment` mixin and `AlignmentCorrector`.
#[derive(Debug, Clone)]
pub struct ParameterAlignment {
    style: Style,
    indentation_width: i64,
    /// RuboCop's `@current_offenses`, scoped to this rule's own offenses in
    /// the current file (see [`super::alignment_mixin::check_alignment`]).
    reported: Vec<Span>,
}

impl ParameterAlignment {
    /// RuboCop's `message`.
    const fn message(&self) -> &'static str {
        match self.style {
            Style::WithFixedIndentation => FIXED_INDENT_MSG,
            Style::WithFirstParameter => ALIGN_PARAMS_MSG,
        }
    }

    /// RuboCop's `base_column`.
    fn base_column(&self, def_keyword_line: u32, first: &Node<'_>, ctx: &Context<'_>) -> i64 {
        if self.style == Style::WithFixedIndentation {
            i64::from(indentation_of_line(ctx, def_keyword_line)) + self.indentation_width
        } else {
            i64::from(ctx.display_column(first.span().start))
        }
    }
}

impl Rule for ParameterAlignment {
    const META: RuleMeta = RuleMeta {
        name: "Layout/ParameterAlignment",
        department: Department::Layout,
        summary: "Align the parameters of a method definition if they span more than one line.",
        explanation: "\
Checks that the parameters on a multi-line method call or definition are
aligned.

To set the alignment of the first argument, use the
`Layout/FirstParameterIndentation` cop.

```ruby
# EnforcedStyle: with_first_parameter (default)

# good

def foo(bar,
        baz)
  123
end

def foo(
  bar,
  baz
)
  123
end

# bad

def foo(bar,
     baz)
  123
end

# bad

def foo(
  bar,
     baz)
  123
end
```

```ruby
# EnforcedStyle: with_fixed_indentation

# good

def foo(bar,
  baz)
  123
end

def foo(
  bar,
  baz
)
  123
end

# bad

def foo(bar,
        baz)
  123
end

# bad

def foo(
  bar,
     baz)
  123
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[
            linter::ConfigOption {
                name: "EnforcedStyle",
                default: linter::ConfigDefault::Str("with_first_parameter"),
                allowed: &["with_first_parameter", "with_fixed_indentation"],
                doc: "Aligns following lines with the first parameter \
                      (`with_first_parameter`) or one indentation level past \
                      the line the method definition starts on \
                      (`with_fixed_indentation`).",
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
physical line inside a misaligned parameter's default value is not
separately protected. The block-comment guard is a per-line `=begin` text
match rather than resolving actual `EmbDoc` comment nodes, matching this
crate's other `Alignment`-based cops.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "with_fixed_indentation" => Style::WithFixedIndentation,
            _ => Style::WithFirstParameter,
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
        let Node::DefNode { .. } = node else { return };
        let def = node.as_def_node().expect("kind matched");
        let Some(parameters) = def.parameters() else { return };
        let items = parameter_list(&parameters);
        if items.len() < 2 {
            return;
        }
        let def_keyword_line = ctx.line_col(def.def_keyword_loc().span().start).line;
        let base_column = self.base_column(def_keyword_line, &items[0], ctx);
        let message = self.message();
        check_alignment(ctx, &Self::META, &items, base_column, message, &mut self.reported);
    }
}

/// RuboCop-AST's `DefNode#arguments` (`children[-2]`, iterated via
/// `Enumerable`): every parameter in source order, regardless of kind.
fn parameter_list<'pr>(parameters: &ruby_ast::node::ParametersNode<'pr>) -> Vec<Node<'pr>> {
    let mut items: Vec<Node<'pr>> = Vec::new();
    items.extend(parameters.requireds().iter());
    items.extend(parameters.optionals().iter());
    if let Some(rest) = parameters.rest() {
        items.push(rest);
    }
    items.extend(parameters.posts().iter());
    items.extend(parameters.keywords().iter());
    if let Some(keyword_rest) = parameters.keyword_rest() {
        items.push(keyword_rest);
    }
    if let Some(block) = parameters.block() {
        items.push(block.as_node());
    }
    items.sort_by_key(|item| item.span().start);
    items
}
