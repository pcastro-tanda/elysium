//! `Performance/ConcurrentMonotonicTime`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/concurrent_monotonic_time.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, is_bare_or_toplevel_const};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `send` node span: the call without a `do`/`{}` block but including a `&blk` argument.
fn send_span(call: &ruby_ast::node::CallNode<'_>) -> ruby_source::Span {
    let mut span = call_span_excluding_block(call);
    if let Some(block) = call.block() {
        if block.kind() == NodeKind::BlockArgumentNode {
            span.end = span.end.max(block.span().end);
        }
    }
    span
}

/// Use `Process.clock_gettime(Process::CLOCK_MONOTONIC)` instead of `Concurrent.monotonic_time`.
#[derive(Debug, Clone)]
pub struct ConcurrentMonotonicTime;

impl Rule for ConcurrentMonotonicTime {
    const META: RuleMeta = RuleMeta {
        name: "Performance/ConcurrentMonotonicTime",
        department: Department::Performance,
        summary: "Use `Process.clock_gettime(Process::CLOCK_MONOTONIC)` instead of `Concurrent.monotonic_time`.",
        explanation: "Identifies places where `Concurrent.monotonic_time` can be replaced by `Process.clock_gettime(Process::CLOCK_MONOTONIC)`.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
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
        if call.is_safe_navigation() || call.name().as_slice() != b"monotonic_time" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !is_bare_or_toplevel_const(&receiver) {
            return;
        }
        let is_concurrent = receiver
            .as_constant_read_node()
            .map(|c| c.name().as_slice() == b"Concurrent")
            .or_else(|| {
                receiver
                    .as_constant_path_node()
                    .map(|p| p.name().is_some_and(|n| n.as_slice() == b"Concurrent"))
            })
            .unwrap_or(false);
        if !is_concurrent {
            return;
        }

        let first_argument: Option<Span> = call
            .arguments()
            .and_then(|args| args.arguments().iter().next().map(|a| a.span()))
            .or_else(|| {
                call.block().filter(|b| b.kind() == NodeKind::BlockArgumentNode).map(|b| b.span())
            });
        let optional_unit_parameter = first_argument
            .map(|span| format!(", {}", String::from_utf8_lossy(ctx.text(span))))
            .unwrap_or_default();
        let prefer = format!("Process.clock_gettime(Process::CLOCK_MONOTONIC{optional_unit_parameter})");
        let span = send_span(&call);
        let message = format!(
            "Use `{prefer}` instead of `{}`.",
            String::from_utf8_lossy(ctx.text(span))
        );
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, prefer.into_bytes())],
            },
        );
    }
}
