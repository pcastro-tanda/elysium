//! `Performance/ConcurrentMonotonicTime`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/concurrent_monotonic_time.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `Process.clock_gettime(Process::CLOCK_MONOTONIC)` instead of `Concurrent.monotonic_time`.
#[derive(Debug, Clone)]
pub struct ConcurrentMonotonicTime;

impl Rule for ConcurrentMonotonicTime {
    const META: RuleMeta = RuleMeta {
        name: "Performance/ConcurrentMonotonicTime",
        department: Department::Performance,
        summary: "Use `Process.clock_gettime(Process::CLOCK_MONOTONIC)` instead of `Concurrent.monotonic_time`.",
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
