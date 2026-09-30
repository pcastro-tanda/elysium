//! `Layout/MultilineHashBraceLayout`, ported from RuboCop's
//! `lib/rubocop/cop/layout/multiline_hash_brace_layout.rb` plus the
//! `MultilineLiteralBraceLayout` mixin and `MultilineLiteralBraceCorrector`
//! it uses, shared with the other three cops in this family in
//! [`super::multiline_literal_brace_layout`].
//!
//! Prism gives an explicit `{...}` hash literal its own [`NodeKind::HashNode`],
//! always carrying real `opening_loc`/`closing_loc`; a braceless
//! bare-keyword-argument list (`foo a: 1`) is instead a distinct
//! [`NodeKind::KeywordHashNode`] this cop never visits. RuboCop's own
//! `implicit_literal?` guard (`!node.loc.begin`), needed upstream because
//! whitequark's `:hash` node type covers both shapes, is therefore always
//! false here and never triggers.

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
    same: "Closing hash brace must be on the same line as the last hash element when \
                opening brace is on the same line as the first hash element.",
    new: "Closing hash brace must be on the line after the last hash element when opening \
               brace is on a separate line from the first hash element.",
    always_new: "Closing hash brace must be on the line after the last hash element.",
    always_same: "Closing hash brace must be on the same line as the last hash element.",
};

/// Checks that the closing brace in a hash literal is either on the same line as the last hash element, or a new line.
#[derive(Debug, Clone)]
pub struct MultilineHashBraceLayout {
    style: Style,
    /// RuboCop's `node.chained?`: spans of nodes that are some call's receiver.
    chained: HashSet<Span>,
    /// RuboCop's `node.argument?`: spans of nodes that are some (non-safe-nav) call's argument.
    arguments: HashSet<Span>,
}

impl Rule for MultilineHashBraceLayout {
    const META: RuleMeta = RuleMeta {
        name: "Layout/MultilineHashBraceLayout",
        department: Department::Layout,
        summary: "Checks that the closing brace in a hash literal is either on the same line as the last hash element, or a new line.",
        explanation: "\
Checks that the closing brace in a hash literal is either on the same line
as the last hash element, or a new line.

When using the `symmetrical` (default) style:

If a hash's opening brace is on the same line as the first element of the
hash, then the closing brace should be on the same line as the last element
of the hash.

If a hash's opening brace is on the line above the first element of the
hash, then the closing brace should be on the line below the last element
of the hash.

When using the `new_line` style, the closing brace of a multi-line hash
literal must be on the line after the last element of the hash.

When using the `same_line` style, the closing brace of a multi-line hash
literal must be on the same line as the last element of the hash.

```ruby
# EnforcedStyle: symmetrical (default)

# bad
{ a: 1,
  b: 2
}

# bad
{
  a: 1,
  b: 2 }

# good
{ a: 1,
  b: 2 }

# good
{
  a: 1,
  b: 2
}
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::HashNode, NodeKind::CallNode],
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
            Node::HashNode { .. } => {
                let hash = node.as_hash_node().expect("kind matched");
                let children: Vec<Node<'_>> = hash.elements().iter().collect();
                let span = node.span();
                let literal = BraceLiteral {
                    opening: Some(hash.opening_loc().span()),
                    closing: hash.closing_loc().span(),
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
