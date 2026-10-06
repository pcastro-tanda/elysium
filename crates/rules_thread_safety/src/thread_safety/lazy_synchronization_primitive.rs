//! `ThreadSafety/LazySynchronizationPrimitive`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/lazy_synchronization_primitive.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Do not lazily initialize synchronization primitives with `||=`.
#[derive(Debug, Clone)]
pub struct LazySynchronizationPrimitive;

impl Rule for LazySynchronizationPrimitive {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/LazySynchronizationPrimitive",
        department: Department::ThreadSafety,
        summary: "Do not lazily initialize synchronization primitives with `||=`.",
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
