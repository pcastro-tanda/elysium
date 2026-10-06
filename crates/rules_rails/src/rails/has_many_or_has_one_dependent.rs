//! `Rails/HasManyOrHasOneDependent`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/has_many_or_has_one_dependent.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Define the dependent option to the has_many and has_one associations.
#[derive(Debug, Clone)]
pub struct HasManyOrHasOneDependent;

impl Rule for HasManyOrHasOneDependent {
    const META: RuleMeta = RuleMeta {
        name: "Rails/HasManyOrHasOneDependent",
        department: Department::Rails,
        summary: "Define the dependent option to the has_many and has_one associations.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let _ = (node, ctx, NodeKind::CallNode);
    }
}
