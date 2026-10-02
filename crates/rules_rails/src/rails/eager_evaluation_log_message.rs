//! `Rails/EagerEvaluationLogMessage`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/eager_evaluation_log_message.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::util;

const MSG: &str = "Pass a block to `Rails.logger.debug`.";

/// Check that blocks are used for interpolated strings passed to
/// `Rails.logger.debug`.
#[derive(Debug, Clone)]
pub struct EagerEvaluationLogMessage;

impl Rule for EagerEvaluationLogMessage {
    const META: RuleMeta = RuleMeta {
        name: "Rails/EagerEvaluationLogMessage",
        department: Department::Rails,
        summary: "Checks that blocks are used for interpolated strings passed to \
                  `Rails.logger.debug`.",
        explanation: "Checks that blocks are used for interpolated strings passed to \
                      `Rails.logger.debug`.\n\nBy default, Rails production environments use \
                      the `:info` log level. At the `:info` log level, `Rails.logger.debug` \
                      statements do not result in log output. However, Ruby must eagerly \
                      evaluate interpolated string arguments passed as method arguments. \
                      Passing a block to `Rails.logger.debug` prevents costly evaluation of \
                      interpolated strings when no output would be produced anyway.\n\n\
                      ```ruby\n# bad\nRails.logger.debug \"The time is #{Time.zone.now}.\"\n\n\
                      # good\nRails.logger.debug { \"The time is #{Time.zone.now}.\" }\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"debug" || call.is_safe_navigation() {
            return;
        }
        // `node.block_literal?`
        if call.block().is_some_and(|block| block.as_block_node().is_some()) {
            return;
        }
        let Some(logger) = call.receiver().and_then(|r| r.as_call_node()) else { return };
        if logger.name().as_slice() != b"logger"
            || logger.is_safe_navigation()
            || !util::parser_args(&logger).is_empty()
        {
            return;
        }
        let Some(rails) = logger.receiver() else { return };
        if !is_bare_or_toplevel_const(&rails) || const_name(&rails).as_deref() != Some("Rails") {
            return;
        }
        let args = util::parser_args(&call);
        let [argument] = args.as_slice() else { return };
        if argument.as_interpolated_string_node().is_none() {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let end = node.span().end;
        let parenthesized = call.opening_loc().is_some();
        let start = if parenthesized { selector.span().end } else { selector.span().end + 1 };
        let range = Span::new(start, end);
        let source = String::from_utf8_lossy(ctx.text(argument.span())).into_owned();
        let replacement =
            if parenthesized { format!(" {{ {source} }}") } else { format!("{{ {source} }}") };
        ctx.report_with_fix(
            &Self::META,
            range,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, replacement.into_bytes())],
            },
        );
    }
}
