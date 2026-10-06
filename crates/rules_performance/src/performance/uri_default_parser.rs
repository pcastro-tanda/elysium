//! `Performance/UriDefaultParser`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/uri_default_parser.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{Node, NodeKind};

/// Use `URI::DEFAULT_PARSER` instead of `URI::Parser.new`.
#[derive(Debug, Clone)]
pub struct UriDefaultParser;

impl Rule for UriDefaultParser {
    const META: RuleMeta = RuleMeta {
        name: "Performance/UriDefaultParser",
        department: Department::Performance,
        summary: "Use `URI::DEFAULT_PARSER` instead of `URI::Parser.new`.",
        explanation: "Identifies places where `URI::Parser.new` can be replaced by \
                      `URI::DEFAULT_PARSER`.\n\n```ruby\n# bad\nURI::Parser.new\n\n# good\n\
                      URI::DEFAULT_PARSER\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    /// `(send (const (const ${nil? cbase} :URI) :Parser) :new)`.
    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation()
            || call.name().as_slice() != b"new"
            || call.arguments().is_some()
        {
            return;
        }
        if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(parser) = receiver.as_constant_path_node() else { return };
        if parser.name().is_none_or(|n| n.as_slice() != b"Parser") {
            return;
        }
        let Some(scope) = parser.parent() else { return };
        let cbase = if let Some(read) = scope.as_constant_read_node() {
            if read.name().as_slice() != b"URI" {
                return;
            }
            false
        } else if let Some(path) = scope.as_constant_path_node() {
            if path.parent().is_some() || path.name().is_none_or(|n| n.as_slice() != b"URI") {
                return;
            }
            true
        } else {
            return;
        };
        let double_colon = if cbase { "::" } else { "" };
        let message = format!(
            "Use `{double_colon}URI::DEFAULT_PARSER` instead of `{double_colon}URI::Parser.new`."
        );
        let span = call_span_excluding_block(&call);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(
                    span,
                    format!("{double_colon}URI::DEFAULT_PARSER").into_bytes(),
                )],
            },
        );
    }
}
