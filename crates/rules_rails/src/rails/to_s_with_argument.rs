//! `Rails/ToSWithArgument`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/to_s_with_argument.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

const MSG: &str = "Use `to_formatted_s` instead.";

/// `minimum_target_rails_version 7.0`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 7.0;

/// `EXTENDED_FORMAT_TYPES`.
const EXTENDED_FORMAT_TYPES: &[&[u8]] = &[
    b"currency",
    b"db",
    b"delimited",
    b"human",
    b"human_size",
    b"inspect",
    b"iso8601",
    b"long",
    b"long_ordinal",
    b"nsec",
    b"number",
    b"percentage",
    b"phone",
    b"rfc822",
    b"rounded",
    b"short",
    b"time",
    b"usec",
];

/// Identifies passing any argument to `#to_s`.
#[derive(Debug, Clone)]
pub struct ToSWithArgument {
    /// Whether the target Rails version reaches `minimum_target_rails_version`;
    /// RuboCop does not run the cop at all otherwise.
    supported: bool,
}

impl Rule for ToSWithArgument {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ToSWithArgument",
        department: Department::Rails,
        summary: "Identifies passing any argument to `#to_s`.",
        explanation: "Identifies passing any argument to `#to_s`.\n\nThis cop is marked as \
                      unsafe because it may detect `#to_s` calls that are not related to Active \
                      Support implementation.\n\n```ruby\n# bad\nobj.to_s(:delimited)\n\n# \
                      good\nobj.to_formatted_s(:delimited)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"to_s" {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let Some(first) = arguments.arguments().iter().next() else { return };
        let Some(symbol) = first.as_symbol_node() else { return };
        if !EXTENDED_FORMAT_TYPES.contains(&symbol.unescaped()) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let span = selector.span();
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, b"to_formatted_s".to_vec())],
            },
        );
    }
}
