//! `Performance/ConstantRegexp`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/constant_regexp.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Finds regular expressions with dynamic components that are all constants.
#[derive(Debug, Clone)]
pub struct ConstantRegexp;

impl Rule for ConstantRegexp {
    const META: RuleMeta = RuleMeta {
        name: "Performance/ConstantRegexp",
        department: Department::Performance,
        summary: "Finds regular expressions with dynamic components that are all constants.",
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
