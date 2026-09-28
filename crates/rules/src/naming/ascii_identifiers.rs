//! `Naming/AsciiIdentifiers`, ported from RuboCop's
//! `lib/rubocop/cop/naming/ascii_identifiers.rb`.
//!
//! Upstream walks `processed_source.tokens` for `tIDENTIFIER`/`tCONSTANT`
//! tokens. This port instead visits the Prism nodes that carry those same
//! bare-identifier and constant-name locations (local/call/def/parameter
//! names for identifiers; constant read/write/path names for constants),
//! since elysium has no independent token stream.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `IDENTIFIER_MSG`.
const IDENTIFIER_MSG: &str = "Use only ascii symbols in identifiers.";
/// RuboCop's `CONSTANT_MSG`.
const CONSTANT_MSG: &str = "Use only ascii symbols in constants.";

/// Checks for non-ascii characters in identifier and constant names.
/// Identifiers are always checked and whether constants are checked
/// can be controlled using `AsciiConstants` config.
#[derive(Debug, Clone)]
pub struct AsciiIdentifiers {
    ascii_constants: bool,
}

/// Node kinds that carry a bare-identifier name location (RuboCop's
/// `tIDENTIFIER` tokens).
const IDENTIFIER_KINDS: &[NodeKind] = &[
    NodeKind::LocalVariableReadNode,
    NodeKind::LocalVariableTargetNode,
    NodeKind::LocalVariableWriteNode,
    NodeKind::LocalVariableAndWriteNode,
    NodeKind::LocalVariableOperatorWriteNode,
    NodeKind::LocalVariableOrWriteNode,
    NodeKind::CallNode,
    NodeKind::CallTargetNode,
    NodeKind::CallAndWriteNode,
    NodeKind::CallOperatorWriteNode,
    NodeKind::CallOrWriteNode,
    NodeKind::DefNode,
    NodeKind::RequiredParameterNode,
    NodeKind::OptionalParameterNode,
    NodeKind::RestParameterNode,
    NodeKind::RequiredKeywordParameterNode,
    NodeKind::OptionalKeywordParameterNode,
    NodeKind::KeywordRestParameterNode,
    NodeKind::BlockParameterNode,
    NodeKind::BlockLocalVariableNode,
];

/// Node kinds that carry a constant-name location (RuboCop's `tCONSTANT`
/// tokens).
const CONSTANT_KINDS: &[NodeKind] = &[
    NodeKind::ConstantReadNode,
    NodeKind::ConstantTargetNode,
    NodeKind::ConstantWriteNode,
    NodeKind::ConstantAndWriteNode,
    NodeKind::ConstantOperatorWriteNode,
    NodeKind::ConstantOrWriteNode,
    NodeKind::ConstantPathNode,
    NodeKind::ConstantPathTargetNode,
];

/// Returns the identifier/constant name span for `node`, or `None` if
/// `node` is not one of the kinds this cop tracks (or the location is
/// absent, e.g. an anonymous `*`/`**`/`&` parameter).
fn name_span(node: &Node<'_>) -> Option<Span> {
    match node {
        Node::LocalVariableReadNode { .. }
        | Node::LocalVariableTargetNode { .. }
        | Node::RequiredParameterNode { .. }
        | Node::BlockLocalVariableNode { .. }
        | Node::ConstantReadNode { .. }
        | Node::ConstantTargetNode { .. } => Some(node.span()),
        Node::LocalVariableWriteNode { .. } => {
            Some(node.as_local_variable_write_node()?.name_loc().span())
        }
        Node::LocalVariableAndWriteNode { .. } => {
            Some(node.as_local_variable_and_write_node()?.name_loc().span())
        }
        Node::LocalVariableOperatorWriteNode { .. } => {
            Some(node.as_local_variable_operator_write_node()?.name_loc().span())
        }
        Node::LocalVariableOrWriteNode { .. } => {
            Some(node.as_local_variable_or_write_node()?.name_loc().span())
        }
        Node::CallNode { .. } => {
            let call = node.as_call_node()?;
            // Prism's `message_loc` for the implicit `[]`/`[]=` operators spans
            // the *entire* bracketed expression (e.g. `foo["😀"]`), not just a
            // bare `[]` token. RuboCop's lexer never emits a `tIDENTIFIER` for
            // these (they lex as bracket tokens), so skip them here too.
            if matches!(call.name().as_slice(), b"[]" | b"[]=") {
                return None;
            }
            Some(call.message_loc()?.span())
        }
        Node::CallTargetNode { .. } => Some(node.as_call_target_node()?.message_loc().span()),
        Node::CallAndWriteNode { .. } => Some(node.as_call_and_write_node()?.message_loc()?.span()),
        Node::CallOperatorWriteNode { .. } => {
            Some(node.as_call_operator_write_node()?.message_loc()?.span())
        }
        Node::CallOrWriteNode { .. } => Some(node.as_call_or_write_node()?.message_loc()?.span()),
        Node::DefNode { .. } => Some(node.as_def_node()?.name_loc().span()),
        Node::OptionalParameterNode { .. } => {
            Some(node.as_optional_parameter_node()?.name_loc().span())
        }
        Node::RestParameterNode { .. } => Some(node.as_rest_parameter_node()?.name_loc()?.span()),
        Node::RequiredKeywordParameterNode { .. } => {
            Some(node.as_required_keyword_parameter_node()?.name_loc().span())
        }
        Node::OptionalKeywordParameterNode { .. } => {
            Some(node.as_optional_keyword_parameter_node()?.name_loc().span())
        }
        Node::KeywordRestParameterNode { .. } => {
            Some(node.as_keyword_rest_parameter_node()?.name_loc()?.span())
        }
        Node::BlockParameterNode { .. } => Some(node.as_block_parameter_node()?.name_loc()?.span()),
        Node::ConstantWriteNode { .. } => Some(node.as_constant_write_node()?.name_loc().span()),
        Node::ConstantAndWriteNode { .. } => {
            Some(node.as_constant_and_write_node()?.name_loc().span())
        }
        Node::ConstantOperatorWriteNode { .. } => {
            Some(node.as_constant_operator_write_node()?.name_loc().span())
        }
        Node::ConstantOrWriteNode { .. } => {
            Some(node.as_constant_or_write_node()?.name_loc().span())
        }
        Node::ConstantPathNode { .. } => Some(node.as_constant_path_node()?.name_loc().span()),
        Node::ConstantPathTargetNode { .. } => {
            Some(node.as_constant_path_target_node()?.name_loc().span())
        }
        _ => None,
    }
}

/// RuboCop's `first_non_ascii_chars`: the byte range (within `text`) of the
/// first maximal run of non-ascii characters, or `None` if `text` is
/// `ascii_only?`.
fn first_non_ascii_range(text: &str) -> Option<(usize, usize)> {
    let mut start = None;
    let mut end = None;
    for (i, c) in text.char_indices() {
        if c.is_ascii() {
            if start.is_some() {
                break;
            }
        } else {
            start.get_or_insert(i);
            end = Some(i + c.len_utf8());
        }
    }
    start.zip(end)
}

impl Rule for AsciiIdentifiers {
    const META: RuleMeta = RuleMeta {
        name: "Naming/AsciiIdentifiers",
        department: Department::Naming,
        summary: "Use only ascii symbols in identifiers and constants.",
        explanation: "\
Checks for non-ascii characters in identifier and constant names.
Identifiers are always checked and whether constants are checked
can be controlled using AsciiConstants config.

```ruby
# bad
def καλημερα # Greek alphabet (non-ascii)
end

# bad
def こんにちはと言う # Japanese character (non-ascii)
end

# bad
def hello_🍣 # Emoji (non-ascii)
end

# good
def say_hello
end

# bad
신장 = 10 # Hangul character (non-ascii)

# good
height = 10
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::LocalVariableReadNode,
            NodeKind::LocalVariableTargetNode,
            NodeKind::LocalVariableWriteNode,
            NodeKind::LocalVariableAndWriteNode,
            NodeKind::LocalVariableOperatorWriteNode,
            NodeKind::LocalVariableOrWriteNode,
            NodeKind::CallNode,
            NodeKind::CallTargetNode,
            NodeKind::CallAndWriteNode,
            NodeKind::CallOperatorWriteNode,
            NodeKind::CallOrWriteNode,
            NodeKind::DefNode,
            NodeKind::RequiredParameterNode,
            NodeKind::OptionalParameterNode,
            NodeKind::RestParameterNode,
            NodeKind::RequiredKeywordParameterNode,
            NodeKind::OptionalKeywordParameterNode,
            NodeKind::KeywordRestParameterNode,
            NodeKind::BlockParameterNode,
            NodeKind::BlockLocalVariableNode,
            NodeKind::ConstantReadNode,
            NodeKind::ConstantTargetNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantAndWriteNode,
            NodeKind::ConstantOperatorWriteNode,
            NodeKind::ConstantOrWriteNode,
            NodeKind::ConstantPathNode,
            NodeKind::ConstantPathTargetNode,
        ],
        config: &[ConfigOption {
            name: "AsciiConstants",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Whether to check constant names too.",
        }],
        blind_spots: "\
Upstream also flags non-ascii characters in symbol literals (whose bare
content lexes as a `tIDENTIFIER`/`tCONSTANT` token) and treats bare `@ivar`/
`$gvar`/`@@cvar` names as never-checked (they are not `tIDENTIFIER`/
`tCONSTANT` tokens either). This port does not visit `SymbolNode`, matching
the never-checked instance/global/class-variable behavior but missing the
symbol-literal case.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { ascii_constants: options.bool("AsciiConstants") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let is_constant = CONSTANT_KINDS.contains(&node.kind());
        if is_constant {
            if !self.ascii_constants {
                return;
            }
        } else if !IDENTIFIER_KINDS.contains(&node.kind()) {
            return;
        }

        let Some(span) = name_span(node) else { return };
        let Ok(text) = std::str::from_utf8(ctx.text(span)) else { return };
        let Some((start, end)) = first_non_ascii_range(text) else { return };

        let Ok(start) = u32::try_from(start) else { return };
        let Ok(end) = u32::try_from(end) else { return };
        let offense_span = Span::new(span.start + start, span.start + end);
        let message = if is_constant { CONSTANT_MSG } else { IDENTIFIER_MSG };
        ctx.report(&Self::META, offense_span, message);
    }
}
