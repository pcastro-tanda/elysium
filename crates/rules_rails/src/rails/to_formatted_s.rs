//! `Rails/ToFormattedS`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/to_formatted_s.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// `minimum_target_rails_version 7.0`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 7.0;

/// Checks for consistent uses of `to_fs` or `to_formatted_s`.
#[derive(Debug, Clone)]
pub struct ToFormattedS {
    supported: bool,
    /// `EnforcedStyle`: the method name to prefer.
    style: &'static str,
}

impl Rule for ToFormattedS {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ToFormattedS",
        department: Department::Rails,
        summary: "Checks for consistent uses of `to_fs` or `to_formatted_s`.",
        explanation: "Checks for consistent uses of `to_fs` or `to_formatted_s`, depending on \
                      the cop's configuration.\n\n```ruby\n# EnforcedStyle: to_fs (default)\n\
                      # bad\ntime.to_formatted_s(:db)\n\n# good\ntime.to_fs(:db)\n```\n\n\
                      ```ruby\n# EnforcedStyle: to_formatted_s\n# bad\ntime.to_fs(:db)\n\n\
                      # good\ntime.to_formatted_s(:db)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("to_fs"),
            allowed: &["to_fs", "to_formatted_s"],
            doc: "Which of the two equivalent methods to use.",
        }],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "to_formatted_s" => "to_formatted_s",
            _ => "to_fs",
        };
        Ok(Self { supported: options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION, style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let name = name.as_slice();
        if (name != b"to_formatted_s" && name != b"to_fs") || name == self.style.as_bytes() {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let span = selector.span();
        ctx.report_with_fix(
            &Self::META,
            span,
            format!("Use `{}` instead.", self.style),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, self.style.as_bytes().to_vec())],
            },
        );
    }
}
