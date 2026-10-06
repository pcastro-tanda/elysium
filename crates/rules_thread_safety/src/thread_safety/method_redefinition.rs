//! `ThreadSafety/MethodRedefinition`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/method_redefinition.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Do not use `remove_method` followed by method definition.
#[derive(Debug, Clone)]
pub struct MethodRedefinition;

impl Rule for MethodRedefinition {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/MethodRedefinition",
        department: Department::ThreadSafety,
        summary: "Do not use `remove_method` followed by method definition.",
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
