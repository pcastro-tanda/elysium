//! `Rails/ResponseParsedBody`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/response_parsed_body.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Prefer `response.parsed_body`.";
const MINIMUM_TARGET_RAILS_VERSION: f64 = 5.0;

/// Prefer `response.parsed_body` to custom parsing logic for `response.body`.
#[derive(Debug, Clone)]
pub struct ResponseParsedBody {
    supported: bool,
    html: bool,
}

impl Rule for ResponseParsedBody {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ResponseParsedBody",
        department: Department::Rails,
        summary: "Prefer `response.parsed_body` to custom parsing logic for `response.body`.",
        explanation: "Prefer `response.parsed_body` to custom parsing logic for `response.body`.\n\n\
                      This cop's autocorrection is unsafe because it assumes the response \
                      content type matches the parser used.\n\n\
                      ```ruby\n# bad\nJSON.parse(response.body)\n\n# good\nresponse.parsed_body\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let version = options.target_rails_version();
        Ok(Self { supported: version >= MINIMUM_TARGET_RAILS_VERSION, html: version >= 7.1 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation()
            || call.block().is_some_and(|b| b.as_block_argument_node().is_some())
        {
            return;
        }
        let name = call.name();
        let name = name.as_slice();
        if !matches!(name, b"parse" | b"HTML" | b"HTML4" | b"HTML5") {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(arguments) = call.arguments() else { return };
        let mut arguments = arguments.arguments().iter();
        let (Some(argument), None) = (arguments.next(), arguments.next()) else { return };
        if !is_response_body(&argument) {
            return;
        }
        let hit = if name == b"parse" {
            is_const(&receiver, "JSON")
                || (self.html
                    && (is_nokogiri_html(&receiver) || is_nokogiri_html_document(&receiver)))
        } else {
            self.html && is_const(&receiver, "Nokogiri")
        };
        if hit {
            let span = node.span();
            ctx.report_with_fix(
                &Self::META,
                span,
                MSG,
                Fix {
                    applicability: Applicability::Unsafe,
                    edits: vec![Edit::replace(span, b"response.parsed_body".to_vec())],
                },
            );
        }
    }
}

/// `(const {nil? cbase} :Name)`.
fn is_const(node: &Node<'_>, name: &str) -> bool {
    is_bare_or_toplevel_const(node) && const_name(node).as_deref() == Some(name)
}

/// `(const #nokogiri? HTML)`.
fn is_nokogiri_html(node: &Node<'_>) -> bool {
    let Some(path) = node.as_constant_path_node() else { return false };
    path.name().is_some_and(|n| matches!(n.as_slice(), b"HTML" | b"HTML4" | b"HTML5"))
        && path.parent().is_some_and(|parent| is_const(&parent, "Nokogiri"))
}

/// `(const #nokogiri_html? :Document)`.
fn is_nokogiri_html_document(node: &Node<'_>) -> bool {
    let Some(path) = node.as_constant_path_node() else { return false };
    path.name().is_some_and(|n| n.as_slice() == b"Document")
        && path.parent().is_some_and(|parent| is_nokogiri_html(&parent))
}

/// `(send (send nil? :response) :body)`.
fn is_response_body(node: &Node<'_>) -> bool {
    let Some(body) = node.as_call_node() else { return false };
    if body.name().as_slice() != b"body"
        || body.is_safe_navigation()
        || body.arguments().is_some()
        || body.block().is_some()
    {
        return false;
    }
    let Some(receiver) = body.receiver() else { return false };
    let Some(response) = receiver.as_call_node() else { return false };
    response.name().as_slice() == b"response"
        && response.receiver().is_none()
        && response.arguments().is_none()
        && response.block().is_none()
}
