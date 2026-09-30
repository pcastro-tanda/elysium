//! `Layout/MultilineMethodDefinitionBraceLayout`, ported from RuboCop's
//! `lib/rubocop/cop/layout/multiline_method_definition_brace_layout.rb` plus
//! the `MultilineLiteralBraceLayout` mixin and `MultilineLiteralBraceCorrector`
//! it uses, shared with the other three cops in this family in
//! [`super::multiline_literal_brace_layout`].
//!
//! RuboCop's `on_def`/`on_defs` call `check_brace_layout(node.arguments)`:
//! the mixin's "node" is the *parameter list*, not the `def` itself, so its
//! own `chained?`/`argument?` (parent-relation) checks always read `false`
//! (a parameter list's parent is always the `def`, never a call), and its
//! `loc.begin`/`loc.end` are the parameter list's own parens. Prism has no
//! equivalent wrapper node -- [`ruby_ast::node::DefNode::lparen_loc`]/
//! `rparen_loc` (`None` for a parenless `def`, even with parameters) and the
//! flattened parameter list stand in directly.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::ParametersNode;
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

use super::multiline_literal_brace_layout::{
    check_brace_layout, resolve_style, BraceLiteral, Messages, Style,
};

const MESSAGES: Messages = Messages {
    same: "Closing method definition brace must be on the same line as the last parameter \
                when opening brace is on the same line as the first parameter.",
    new: "Closing method definition brace must be on the line after the last parameter \
               when opening brace is on a separate line from the first parameter.",
    always_new: "Closing method definition brace must be on the line after the last \
                       parameter.",
    always_same: "Closing method definition brace must be on the same line as the last \
                        parameter.",
};

/// Checks that the closing brace in a method definition is either on the same line as the last method parameter, or a new line.
#[derive(Debug, Clone)]
pub struct MultilineMethodDefinitionBraceLayout {
    style: Style,
}

impl Rule for MultilineMethodDefinitionBraceLayout {
    const META: RuleMeta = RuleMeta {
        name: "Layout/MultilineMethodDefinitionBraceLayout",
        department: Department::Layout,
        summary: "Checks that the closing brace in a method definition is either on the same line as the last method parameter, or a new line.",
        explanation: "\
Checks that the closing brace in a method definition is either on the same
line as the last method parameter, or a new line.

When using the `symmetrical` (default) style:

If a method definition's opening brace is on the same line as the first
parameter of the definition, then the closing brace should be on the same
line as the last parameter of the definition.

If a method definition's opening brace is on the line above the first
parameter of the definition, then the closing brace should be on the line
below the last parameter of the definition.

When using the `new_line` style, the closing brace of a multi-line method
definition must be on the line after the last parameter of the definition.

When using the `same_line` style, the closing brace of a multi-line method
definition must be on the same line as the last parameter of the definition.

```ruby
# EnforcedStyle: symmetrical (default)

# bad
def foo(a,
  b
)
end

# bad
def foo(
  a,
  b)
end

# good
def foo(a,
  b)
end

# good
def foo(
  a,
  b
)
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("symmetrical"),
            allowed: &["symmetrical", "new_line", "same_line"],
            doc: "Whether the closing brace mirrors the opening brace's own \
                  line (`symmetrical`), always sits on the line after the \
                  last parameter (`new_line`), or always sits on the same \
                  line as the last parameter (`same_line`).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { style: resolve_style(options)? })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Node::DefNode { .. } = node else { return };
        let def = node.as_def_node().expect("kind matched");
        let Some(lparen) = def.lparen_loc() else { return };
        let Some(rparen) = def.rparen_loc() else { return };
        let opening = lparen.span();
        let closing = rparen.span();
        let literal = BraceLiteral {
            opening: Some(opening),
            closing,
            children: def_parameter_list(def.parameters()),
            whole_span: Span::new(opening.start, closing.end),
            chained_or_argument: false,
            heredoc_chain: None,
        };
        check_brace_layout(ctx, &Self::META, self.style, &MESSAGES, &literal);
    }
}

/// Every entry in a `def`'s parameter list, in declaration order. RuboCop-AST's
/// `DefNode#arguments`, which whitequark's own args node exposes as a flat
/// child list (Ruby's grammar fixes `requireds, optionals, rest, posts,
/// keywords, keyword_rest, block` as the only legal order).
fn def_parameter_list(params: Option<ParametersNode<'_>>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    let Some(params) = params else { return out };
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
