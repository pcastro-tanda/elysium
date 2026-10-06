//! `Rails/HasAndBelongsToMany`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/has_and_belongs_to_many.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

const MSG: &str = "Prefer `has_many :through` to `has_and_belongs_to_many`.";

/// Checks for the use of the `has_and_belongs_to_many` macro.
#[derive(Debug, Clone)]
pub struct HasAndBelongsToMany;

impl Rule for HasAndBelongsToMany {
    const META: RuleMeta = RuleMeta {
        name: "Rails/HasAndBelongsToMany",
        department: Department::Rails,
        summary: "Prefer has_many :through to has_and_belongs_to_many.",
        explanation: "Checks for the use of the `has_and_belongs_to_many` macro.\n\n```ruby\n\
                      # bad\nhas_and_belongs_to_many :ingredients\n\n# good\nhas_many \
                      :ingredients, through: :recipe_ingredients\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // `node.command?(:has_and_belongs_to_many)`: no receiver.
        if call.receiver().is_some() || call.name().as_slice() != b"has_and_belongs_to_many" {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        ctx.report(&Self::META, selector.span(), MSG);
    }
}
