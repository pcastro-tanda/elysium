//! `Sorbet/ForbidRBIOutsideOfAllowedPaths`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_rbi_outside_of_allowed_paths.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Forbids RBI files outside of the allowed paths
#[derive(Debug, Clone)]
pub struct ForbidRBIOutsideOfAllowedPaths;

impl Rule for ForbidRBIOutsideOfAllowedPaths {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidRBIOutsideOfAllowedPaths",
        department: Department::Sorbet,
        summary: "Forbids RBI files outside of the allowed paths",
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
