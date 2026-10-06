//! `Rails/StripHeredoc`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/strip_heredoc.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, is_heredoc};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use squiggly heredoc (`<<~`) instead of `strip_heredoc`.";

/// Enforces the use of squiggly heredoc over `strip_heredoc`.
#[derive(Debug, Clone)]
pub struct StripHeredoc {
    /// `minimum_target_ruby_version 2.3`.
    supported: bool,
}

impl Rule for StripHeredoc {
    const META: RuleMeta = RuleMeta {
        name: "Rails/StripHeredoc",
        department: Department::Rails,
        summary: "Enforces the use of squiggly heredoc over strip_heredoc.",
        explanation: "Enforces the use of squiggly heredoc over `strip_heredoc`.\n\n```ruby\n# \
                      bad\n<<EOS.strip_heredoc\n  some text\nEOS\n\n# bad\n<<-EOS.strip_heredoc\n  \
                      some text\nEOS\n\n# good\n<<~EOS\n  some text\nEOS\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: options.target_ruby_version() >= 2.3 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() || call.name().as_slice() != b"strip_heredoc" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        // `receiver.type?(:str, :dstr)` and `heredoc?` (xstr heredocs excluded).
        if !(receiver.as_string_node().is_some()
            || receiver.as_interpolated_string_node().is_some())
            || !is_heredoc(&receiver)
        {
            return;
        }
        let opening = if let Some(string) = receiver.as_string_node() {
            string.opening_loc()
        } else {
            receiver.as_interpolated_string_node().and_then(|string| string.opening_loc())
        };
        let Some(opening) = opening else { return };
        let opening = opening.span();
        let text = ctx.text(opening);
        let rest = text.strip_prefix(b"<<").unwrap_or(text);
        let rest = rest.strip_prefix(b"-").or_else(|| rest.strip_prefix(b"~")).unwrap_or(rest);
        let mut squiggly = b"<<~".to_vec();
        squiggly.extend_from_slice(rest);

        let mut edits = vec![Edit::replace(opening, squiggly)];
        if let Some(dot) = call.call_operator_loc() {
            edits.push(Edit::delete(dot.span()));
        }
        if let Some(selector) = call.message_loc() {
            edits.push(Edit::delete(selector.span()));
        }
        let span: Span = call_span_excluding_block(&call);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}
