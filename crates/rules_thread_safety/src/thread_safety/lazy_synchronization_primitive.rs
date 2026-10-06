//! `ThreadSafety/LazySynchronizationPrimitive`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/lazy_synchronization_primitive.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Do not lazily initialize synchronization primitives with `||=`.";

/// Do not lazily initialize synchronization primitives with `||=`.
#[derive(Debug, Clone)]
pub struct LazySynchronizationPrimitive {
    /// Spans of `synchronize` blocks entered so far (pre-order, so enclosing ones come first).
    synchronize_blocks: Vec<Span>,
}

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
        kinds: &[NodeKind::InstanceVariableOrWriteNode, NodeKind::ClassVariableOrWriteNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { synchronize_blocks: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(call) = node.as_call_node() {
            // `ancestor.method?(:synchronize)`: only the call's own block counts.
            if call.name().as_slice() == b"synchronize" {
                if let Some(block) = call.block().filter(|b| b.as_block_node().is_some()) {
                    self.synchronize_blocks.push(block.span());
                }
            }
            return;
        }
        let value = if let Some(w) = node.as_instance_variable_or_write_node() {
            w.value()
        } else if let Some(w) = node.as_class_variable_or_write_node() {
            w.value()
        } else {
            return;
        };
        if !is_synchronization_primitive(&value) {
            return;
        }
        if !ctx.ancestors().iter().any(|a| a.kind == NodeKind::DefNode) {
            return;
        }
        let span = node.span();
        if self.synchronize_blocks.iter().any(|b| b.start <= span.start && span.end <= b.end) {
            return;
        }
        ctx.report(&Self::META, span, MSG);
    }
}

fn is_synchronization_primitive(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.is_safe_navigation()
        || call.name().as_slice() != b"new"
        || call.block().is_some_and(|b| b.as_block_node().is_some())
    {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    if let Some(constant) = receiver.as_constant_read_node() {
        return is_mutex_or_monitor(constant.name().as_slice());
    }
    let Some(path) = receiver.as_constant_path_node() else { return false };
    let Some(name) = path.name() else { return false };
    match path.parent() {
        None => is_mutex_or_monitor(name.as_slice()),
        Some(parent) => {
            name.as_slice() == b"Mutex"
                && parent.as_constant_read_node().is_some_and(|p| p.name().as_slice() == b"Thread")
        }
    }
}

fn is_mutex_or_monitor(name: &[u8]) -> bool {
    name == b"Mutex" || name == b"Monitor"
}
