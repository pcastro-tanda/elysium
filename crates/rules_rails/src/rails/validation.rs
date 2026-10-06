//! `Rails/Validation`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/validation.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use validates :attribute, hash of validations.
#[derive(Debug, Clone)]
pub struct Validation;

impl Rule for Validation {
    const META: RuleMeta = RuleMeta {
        name: "Rails/Validation",
        department: Department::Rails,
        summary: "Use validates :attribute, hash of validations.",
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
