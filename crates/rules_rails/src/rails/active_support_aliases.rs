//! `Rails/ActiveSupportAliases`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/active_support_aliases.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// `ALIASES`: aliased method, original method, and whether the receiver must
/// be a string (otherwise an array) literal.
const ALIASES: [(&str, &str, bool); 4] = [
    ("starts_with?", "start_with?", true),
    ("ends_with?", "end_with?", true),
    ("append", "<<", false),
    ("prepend", "unshift", false),
];

/// Checks that ActiveSupport aliases to core ruby methods are not used.
#[derive(Debug, Clone)]
pub struct ActiveSupportAliases;

impl Rule for ActiveSupportAliases {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ActiveSupportAliases",
        department: Department::Rails,
        summary: "Avoid ActiveSupport aliases of standard ruby methods: `String#starts_with?`, \
                  `String#ends_with?`, `Array#append`, `Array#prepend`.",
        explanation: "Checks that ActiveSupport aliases to core ruby methods are not used.\n\n\
                      ```ruby\n# good\n'some_string'.start_with?('prefix')\n\
                      'some_string'.end_with?('suffix')\n[1, 2, 'a'] << 'b'\n\
                      [1, 2, 'a'].unshift('b')\n\n# bad\n'some_string'.starts_with?('prefix')\n\
                      'some_string'.ends_with?('suffix')\n[1, 2, 'a'].append('b')\n\
                      [1, 2, 'a'].prepend('b')\n```",
        enabled_by_default: true,
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
        let name = call.name();
        let Some(&(current, preferred, string_receiver)) =
            ALIASES.iter().find(|(alias, ..)| alias.as_bytes() == name.as_slice())
        else {
            return;
        };
        let Some(receiver) = call.receiver() else { return };
        let receiver_matches = if string_receiver {
            receiver.as_string_node().is_some()
        } else {
            receiver.as_array_node().is_some()
        };
        if !receiver_matches {
            return;
        }
        // `_`: exactly one argument (a block-pass counts as one in whitequark).
        let block_pass = call.block().is_some_and(|block| block.as_block_argument_node().is_some());
        let argument_count =
            call.arguments().map_or(0, |arguments| arguments.arguments().iter().count())
                + usize::from(block_pass);
        if argument_count != 1 {
            return;
        }
        let Some(selector) = call.message_loc().map(|loc| loc.span()) else { return };

        let message = format!("Use `{preferred}` instead of `{current}`.");
        let span = Span::new(selector.start, call_span_excluding_block(&call).end);
        if current == "append" {
            ctx.report(&Self::META, span, message);
        } else {
            ctx.report_with_fix(
                &Self::META,
                span,
                message,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(selector, preferred.as_bytes().to_vec())],
                },
            );
        }
    }
}
