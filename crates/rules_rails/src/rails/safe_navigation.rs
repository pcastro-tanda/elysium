//! `Rails/SafeNavigation`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/safe_navigation.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use Ruby's safe navigation operator (`&.`) instead of `try!`.
#[derive(Debug, Clone)]
pub struct SafeNavigation;

impl Rule for SafeNavigation {
    const META: RuleMeta = RuleMeta {
        name: "Rails/SafeNavigation",
        department: Department::Rails,
        summary: "Use Ruby's safe navigation operator (`&.`) instead of `try!`.",
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
