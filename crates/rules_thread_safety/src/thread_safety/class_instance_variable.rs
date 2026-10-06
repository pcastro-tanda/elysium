//! `ThreadSafety/ClassInstanceVariable`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/class_instance_variable.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Avoid class instance variables.
#[derive(Debug, Clone)]
pub struct ClassInstanceVariable;

impl Rule for ClassInstanceVariable {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/ClassInstanceVariable",
        department: Department::ThreadSafety,
        summary: "Avoid class instance variables.",
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
