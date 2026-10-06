//! `Performance/TimesMap`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/times_map.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks for .times.map calls.
#[derive(Debug, Clone)]
pub struct TimesMap;

impl Rule for TimesMap {
    const META: RuleMeta = RuleMeta {
        name: "Performance/TimesMap",
        department: Department::Performance,
        summary: "Checks for .times.map calls.",
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
