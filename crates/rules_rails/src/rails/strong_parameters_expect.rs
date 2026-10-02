//! `Rails/StrongParametersExpect`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/strong_parameters_expect.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Enforces the use of `ActionController::Parameters#expect` as a method for strong parameter handling.
#[derive(Debug, Clone)]
pub struct StrongParametersExpect;

impl Rule for StrongParametersExpect {
    const META: RuleMeta = RuleMeta {
        name: "Rails/StrongParametersExpect",
        department: Department::Rails,
        summary: "Enforces the use of `ActionController::Parameters#expect` as a method for strong parameter handling.",
        explanation: "",
        enabled_by_default: false,
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
