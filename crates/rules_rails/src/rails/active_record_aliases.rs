//! `Rails/ActiveRecordAliases`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/active_record_aliases.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// `ALIASES`: `(current, preferred)`.
const ALIASES: [(&str, &str); 2] =
    [("update_attributes", "update"), ("update_attributes!", "update!")];

/// Avoid Active Record aliases: Use `update` instead of `update_attributes`. Use `update!` instead of `update_attributes!`.
#[derive(Debug, Clone)]
pub struct ActiveRecordAliases;

impl Rule for ActiveRecordAliases {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ActiveRecordAliases",
        department: Department::Rails,
        summary: "Avoid Active Record aliases: Use `update` instead of `update_attributes`. Use `update!` instead of `update_attributes!`.",
        explanation: "Checks that ActiveRecord aliases are not used. The direct method names are \
                      more clear and easier to read.\n\nThis cop is unsafe because custom \
                      `update_attributes` method call was changed to `update` but the method \
                      name remained same in the method definition.\n\n```ruby\n# bad\n\
                      book.update_attributes!(author: 'Alice')\n\n# good\n\
                      book.update!(author: 'Alice')\n```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
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
        let Some(&(current, prefer)) =
            ALIASES.iter().find(|(alias, _)| alias.as_bytes() == name.as_slice())
        else {
            return;
        };
        // A `&block` argument counts as an argument in whitequark.
        if call.arguments().is_none() && call.block().is_none_or(|b| b.as_block_node().is_some()) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let span = selector.span();
        ctx.report_with_fix(
            &Self::META,
            span,
            format!("Use `{prefer}` instead of `{current}`."),
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, prefer.as_bytes().to_vec())],
            },
        );
    }
}
