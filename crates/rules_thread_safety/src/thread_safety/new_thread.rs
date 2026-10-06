//! `ThreadSafety/NewThread`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/new_thread.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::{call_span_excluding_block, const_name, is_bare_or_toplevel_const};
use ruby_ast::{Node, NodeKind};

const MSG: &str = "Avoid starting new threads.";

/// Avoid starting new threads. Let a framework like Sidekiq handle the threads.
#[derive(Debug, Clone)]
pub struct NewThread;

impl Rule for NewThread {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/NewThread",
        department: Department::ThreadSafety,
        summary: "Avoid starting new threads. Let a framework like Sidekiq handle the threads.",
        explanation: "Avoid starting new threads.\n\nLet a framework like Sidekiq handle the \
                      threads.\n\n```ruby\n# bad\nThread.new { do_work }\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if !matches!(call.name().as_slice(), b"new" | b"fork" | b"start") {
            return;
        }
        // `(call (const {nil? cbase} :Thread) ...)`: `.` and `&.` alike.
        let Some(receiver) = call.receiver() else { return };
        if !is_bare_or_toplevel_const(&receiver)
            || const_name(&receiver).as_deref() != Some("Thread")
        {
            return;
        }
        ctx.report(&Self::META, call_span_excluding_block(&call), MSG);
    }
}
