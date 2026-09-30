//! `Layout/MultilineArrayBraceLayout`, ported from RuboCop's
//! `lib/rubocop/cop/layout/multiline_array_brace_layout.rb` plus the
//! `MultilineLiteralBraceLayout` mixin and `MultilineLiteralBraceCorrector`
//! it uses, shared with the other three cops in this family in
//! [`super::multiline_literal_brace_layout`].
//!
//! An explicit `[...]` array literal always carries both delimiter
//! locations; only an *implicit* array (a multiple-assignment right-hand
//! side with no brackets, e.g. `a, b = 1, 2`) has neither, matching
//! RuboCop's `implicit_literal?` for free.

use std::collections::HashSet;

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::multiline_literal_brace_layout::{
    check_brace_layout, record_call_relations, resolve_style, BraceLiteral, Messages, Style,
};

const MESSAGES: Messages = Messages {
    same: "The closing array brace must be on the same line as the last array element \
                when the opening brace is on the same line as the first array element.",
    new: "The closing array brace must be on the line after the last array element when \
               the opening brace is on a separate line from the first array element.",
    always_new: "The closing array brace must be on the line after the last array element.",
    always_same: "The closing array brace must be on the same line as the last array element.",
};

/// Checks that the closing brace in an array literal is either on the same line as the last array element, or a new line.
#[derive(Debug, Clone)]
pub struct MultilineArrayBraceLayout {
    style: Style,
    /// RuboCop's `node.chained?`: spans of nodes that are some call's receiver.
    chained: HashSet<Span>,
    /// RuboCop's `node.argument?`: spans of nodes that are some (non-safe-nav) call's argument.
    arguments: HashSet<Span>,
}

impl Rule for MultilineArrayBraceLayout {
    const META: RuleMeta = RuleMeta {
        name: "Layout/MultilineArrayBraceLayout",
        department: Department::Layout,
        summary: "Checks that the closing brace in an array literal is either on the same line as the last array element, or a new line.",
        explanation: "\
Checks that the closing brace in an array literal is either on the same line
as the last array element, or a new line.

When using the `symmetrical` (default) style:

If an array's opening brace is on the same line as the first element of the
array, then the closing brace should be on the same line as the last element
of the array.

If an array's opening brace is on the line above the first element of the
array, then the closing brace should be on the line below the last element
of the array.

When using the `new_line` style, the closing brace of a multi-line array
literal must be on the line after the last element of the array.

When using the `same_line` style, the closing brace of a multi-line array
literal must be on the same line as the last element of the array.

```ruby
# EnforcedStyle: symmetrical (default)

# bad
[ :a,
  :b
]

# bad
[
  :a,
  :b ]

# good
[ :a,
  :b ]

# good
[
  :a,
  :b
]
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ArrayNode, NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("symmetrical"),
            allowed: &["symmetrical", "new_line", "same_line"],
            doc: "Whether the closing brace mirrors the opening brace's own \
                  line (`symmetrical`), always sits on the line after the \
                  last element (`new_line`), or always sits on the same \
                  line as the last element (`same_line`).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            style: resolve_style(options)?,
            chained: HashSet::new(),
            arguments: HashSet::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.chained.clear();
        self.arguments.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                record_call_relations(&call, &mut self.chained, &mut self.arguments);
            }
            Node::ArrayNode { .. } => {
                let array = node.as_array_node().expect("kind matched");
                let Some(opening) = array.opening_loc() else { return };
                let Some(closing) = array.closing_loc() else { return };
                let children: Vec<Node<'_>> = array.elements().iter().collect();
                let span = node.span();
                let literal = BraceLiteral {
                    opening: Some(opening.span()),
                    closing: closing.span(),
                    children,
                    whole_span: span,
                    chained_or_argument: self.chained.contains(&span)
                        || self.arguments.contains(&span),
                    heredoc_chain: None,
                };
                check_brace_layout(ctx, &Self::META, self.style, &MESSAGES, &literal);
            }
            _ => {}
        }
    }
}
