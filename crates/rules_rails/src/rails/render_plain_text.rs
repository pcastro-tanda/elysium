//! `Rails/RenderPlainText`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/render_plain_text.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Prefer `render plain:` over `render text:`.";

/// Prefer `render plain:` over `render text:`.
#[derive(Debug, Clone)]
pub struct RenderPlainText {
    content_type_compatibility: bool,
}

impl Rule for RenderPlainText {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RenderPlainText",
        department: Department::Rails,
        summary: "Prefer `render plain:` over `render text:`.",
        explanation: "Identifies places where `render text:` can be replaced with `render \
                      plain:`.\n\n```ruby\n# bad - explicit MIME type to `text/plain`\nrender \
                      text: 'Ruby!', content_type: 'text/plain'\n\n# good - short and \
                      precise\nrender plain: 'Ruby!'\n\n# good - explicit MIME type not to \
                      `text/plain`\nrender text: 'Ruby!', content_type: 'text/html'\n```\n\nWith \
                      `ContentTypeCompatibility: true` (default), `render text: 'Ruby!'` is \
                      left alone because it sets the MIME type to `text/html`; with `false` it \
                      is flagged too.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "ContentTypeCompatibility",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Convert only when `content_type` is explicitly set to `text/plain`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { content_type_compatibility: options.bool("ContentTypeCompatibility") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // `(send nil? :render $(hash ...))`: a lone hash argument (an `&block`
        // argument would be a second `send` argument in whitequark).
        if call.name().as_slice() != b"render"
            || call.receiver().is_some()
            || call.block().is_some_and(|block| block.as_block_argument_node().is_some())
        {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut arguments = arguments.arguments().iter();
        let (Some(options), None) = (arguments.next(), arguments.next()) else { return };
        let elements: Vec<Node<'_>> = match options.kind() {
            NodeKind::HashNode => options.as_hash_node().map(|h| h.elements().iter().collect()),
            NodeKind::KeywordHashNode => {
                options.as_keyword_hash_node().map(|h| h.elements().iter().collect())
            }
            _ => None,
        }
        .unwrap_or_default();
        let pairs: Vec<_> = elements.iter().filter_map(ruby_ast::Node::as_assoc_node).collect();

        let Some(text_pair) = pairs.iter().find(|pair| key_is_sym(&pair.key(), b"text")) else {
            return;
        };
        let content_type = pairs.iter().find(|pair| key_is_value(&pair.key(), b"content_type"));
        if !self.compatible_content_type(content_type.map(ruby_ast::node::AssocNode::value)) {
            return;
        }

        let rest: Vec<&[u8]> = pairs
            .iter()
            .filter(|pair| {
                let span = pair.as_node().span();
                span != text_pair.as_node().span()
                    && content_type.is_none_or(|ct| ct.as_node().span() != span)
            })
            .map(|pair| ctx.text(pair.as_node().span()))
            .collect();
        let mut replacement = b"render plain: ".to_vec();
        replacement.extend_from_slice(ctx.text(text_pair.value().span()));
        for source in rest {
            replacement.extend_from_slice(b", ");
            replacement.extend_from_slice(source);
        }
        let span = call_span_excluding_block(&call);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement)],
            },
        );
    }
}

impl RenderPlainText {
    /// `compatible_content_type?`.
    fn compatible_content_type(&self, value: Option<Node<'_>>) -> bool {
        match value {
            None => !self.content_type_compatibility,
            Some(value) => value.as_string_node().is_some_and(|s| s.unescaped() == b"text/plain"),
        }
    }
}

/// `(sym :name)`.
fn key_is_sym(key: &Node<'_>, name: &[u8]) -> bool {
    key.as_symbol_node().is_some_and(|sym| sym.unescaped() == name)
}

/// `key.value.to_sym == :name` for a symbol or string key.
fn key_is_value(key: &Node<'_>, name: &[u8]) -> bool {
    key_is_sym(key, name) || key.as_string_node().is_some_and(|s| s.unescaped() == name)
}
