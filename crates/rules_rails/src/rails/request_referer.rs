//! `Rails/RequestReferer`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/request_referer.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks for consistent uses of `request.referer` or `request.referrer`,
/// depending on the cop's configuration.
#[derive(Debug, Clone)]
pub struct RequestReferer {
    /// `EnforcedStyle`: the spelling that is preferred.
    referer: bool,
}

impl Rule for RequestReferer {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RequestReferer",
        department: Department::Rails,
        summary: "Use consistent syntax for request.referer.",
        explanation: "Checks for consistent uses of `request.referer` or\n`request.referrer`, \
                      depending on the cop's configuration.\n\n```ruby\n# EnforcedStyle: referer \
                      (default)\n# bad\nrequest.referrer\n\n# good\nrequest.referer\n\n# \
                      EnforcedStyle: referrer\n# bad\nrequest.referer\n\n# good\n\
                      request.referrer\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("referer"),
            allowed: &["referer", "referrer"],
            doc: "Which spelling of `request.referer` is enforced.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { referer: options.style("EnforcedStyle")? == "referer" })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        // `(send (send nil? :request) {:referer :referrer})`.
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() || call.arguments().is_some() {
            return;
        }
        let (current, preferred) =
            if self.referer { ("referrer", "referer") } else { ("referer", "referrer") };
        if call.name().as_slice() != current.as_bytes() {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(request) = receiver.as_call_node() else { return };
        if request.receiver().is_some()
            || request.arguments().is_some()
            || request.name().as_slice() != b"request"
        {
            return;
        }
        let span = ruby_ast::ext::call_span_excluding_block(&call);
        ctx.report_with_fix(
            &Self::META,
            span,
            format!("Use `request.{preferred}` instead of `request.{current}`."),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, format!("request.{preferred}").into_bytes())],
            },
        );
    }
}
