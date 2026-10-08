//! `Rails/UnusedRenderContent`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/unused_render_content.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Do not specify body content for a response with a non-content status code";

/// `Set[*100..199, 204, 205, 304] & Rack::Utils::SYMBOL_TO_STATUS_CODE.values`
/// (Rack 2.2).
const NON_CONTENT_STATUS_CODES: [i32; 7] = [100, 101, 102, 103, 204, 205, 304];
/// The symbols of `NON_CONTENT_STATUS_CODES`.
const NON_CONTENT_STATUSES: [&[u8]; 7] = [
    b"continue",
    b"switching_protocols",
    b"processing",
    b"early_hints",
    b"no_content",
    b"reset_content",
    b"not_modified",
];
const BODY_OPTIONS: [&[u8]; 14] = [
    b"action",
    b"body",
    b"content_type",
    b"file",
    b"html",
    b"inline",
    b"json",
    b"js",
    b"layout",
    b"plain",
    b"raw",
    b"template",
    b"text",
    b"xml",
];

/// Checks for `render` calls that specify both body content and a
/// non-content status.
#[derive(Debug, Clone)]
pub struct UnusedRenderContent;

impl Rule for UnusedRenderContent {
    const META: RuleMeta = RuleMeta {
        name: "Rails/UnusedRenderContent",
        department: Department::Rails,
        summary: "Do not specify body content for a response with a non-content status code.",
        explanation: "If you try to render content along with a non-content status code \
                      (100-199, 204, 205, or 304), it will be dropped from the response.\n\n\
                      This cop checks for uses of `render` which specify both body content and \
                      a non-content status.\n\n```ruby\n# bad\nrender 'foo', status: \
                      :continue\nrender status: 100, plain: 'Ruby!'\n\n# good\nhead \
                      :continue\nhead 100\n```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "The non-content status symbols are those of Rack 2.2 and later.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"render"
            || call.receiver().is_some()
            || call.is_safe_navigation()
            || call.block().is_some_and(|b| b.as_block_argument_node().is_some())
        {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let arguments: Vec<Node<'_>> = arguments.arguments().iter().collect();
        match arguments.as_slice() {
            [only] => {
                let Some(elements) = hash_elements(only) else { return };
                if !elements.iter().any(is_non_content_status) {
                    return;
                }
                if let Some(body) = elements.iter().find(|e| is_body_pair(e)) {
                    ctx.report(&Self::META, body.span(), MSG);
                }
            }
            [first, second] => {
                if !matches!(first.kind(), NodeKind::StringNode | NodeKind::SymbolNode) {
                    return;
                }
                let Some(elements) = hash_elements(second) else { return };
                if elements.iter().any(is_non_content_status) {
                    ctx.report(&Self::META, first.span(), MSG);
                }
            }
            _ => {}
        }
    }
}

fn hash_elements<'pr>(node: &Node<'pr>) -> Option<Vec<Node<'pr>>> {
    match node.kind() {
        NodeKind::HashNode => Some(node.as_hash_node()?.elements().iter().collect()),
        NodeKind::KeywordHashNode => Some(node.as_keyword_hash_node()?.elements().iter().collect()),
        _ => None,
    }
}

/// `(pair (sym key) value)`: the key's name and the value.
fn symbol_pair<'pr>(node: &Node<'pr>) -> Option<(Vec<u8>, Node<'pr>)> {
    let pair = node.as_assoc_node()?;
    let key = pair.key();
    let key = key.as_symbol_node()?;
    Some((key.unescaped().to_vec(), pair.value()))
}

/// `(pair (sym :status) {(sym NON_CONTENT_STATUSES) (int NON_CONTENT_STATUS_CODES)})`
fn is_non_content_status(node: &Node<'_>) -> bool {
    let Some((key, value)) = symbol_pair(node) else { return false };
    if key != b"status" {
        return false;
    }
    if let Some(symbol) = value.as_symbol_node() {
        return NON_CONTENT_STATUSES.contains(&symbol.unescaped());
    }
    value.as_integer_node().is_some_and(|int| {
        TryInto::<i32>::try_into(int.value())
            .is_ok_and(|code| NON_CONTENT_STATUS_CODES.contains(&code))
    })
}

/// `(pair (sym BODY_OPTIONS) _)`
fn is_body_pair(node: &Node<'_>) -> bool {
    symbol_pair(node).is_some_and(|(key, _)| BODY_OPTIONS.contains(&key.as_slice()))
}
