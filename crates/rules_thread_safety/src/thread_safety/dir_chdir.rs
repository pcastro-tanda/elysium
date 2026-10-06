//! `ThreadSafety/DirChdir`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/dir_chdir.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Avoid using `Dir.chdir` due to its process-wide effect.
#[derive(Debug, Clone)]
pub struct DirChdir;

impl Rule for DirChdir {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/DirChdir",
        department: Department::ThreadSafety,
        summary: "Avoid using `Dir.chdir` due to its process-wide effect.",
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
