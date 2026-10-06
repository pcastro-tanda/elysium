//! `ThreadSafety/ClassAndModuleAttributes`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/class_and_module_attributes.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Avoid mutating class and module attributes.
#[derive(Debug, Clone)]
pub struct ClassAndModuleAttributes;

impl Rule for ClassAndModuleAttributes {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/ClassAndModuleAttributes",
        department: Department::ThreadSafety,
        summary: "Avoid mutating class and module attributes.",
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
