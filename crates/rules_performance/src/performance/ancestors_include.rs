//! `Performance/AncestorsInclude`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/ancestors_include.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `A <= B` instead of `A.ancestors.include?(B)`.
#[derive(Debug, Clone)]
pub struct AncestorsInclude;

impl Rule for AncestorsInclude {
    const META: RuleMeta = RuleMeta {
        name: "Performance/AncestorsInclude",
        department: Department::Performance,
        summary: "Use `A <= B` instead of `A.ancestors.include?(B)`.",
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
