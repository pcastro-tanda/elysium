//! `Performance/ConcurrentMonotonicTime`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/concurrent_monotonic_time.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, is_bare_or_toplevel_const};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
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

/// A call target of an op-assign (`a.b ||= 1`): whitequark's inner `send`.
/// Returns receiver, method name, `.` span, and the send's span.
#[allow(clippy::type_complexity)]
fn op_assign_send<'pr>(
    node: &Node<'pr>,
    allow_safe_navigation: bool,
) -> Option<(Node<'pr>, Vec<u8>, Option<ruby_source::Span>, ruby_source::Span)> {
    macro_rules! parts {
        ($n:expr) => {{
            let n = $n;
            if n.is_safe_navigation() && !allow_safe_navigation {
                return None;
            }
            (
                n.receiver()?,
                n.read_name().as_slice().to_vec(),
                n.call_operator_loc().map(|l| l.span()),
                n.message_loc()?,
            )
        }};
    }
    let (receiver, name, dot, message) = match node.kind() {
        NodeKind::CallOrWriteNode => parts!(node.as_call_or_write_node()?),
        NodeKind::CallAndWriteNode => parts!(node.as_call_and_write_node()?),
        NodeKind::CallOperatorWriteNode => parts!(node.as_call_operator_write_node()?),
        _ => return None,
    };
    let span = ruby_source::Span::new(receiver.span().start, message.span().end);
    Some((receiver, name, dot, span))
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
        kinds: &[
            NodeKind::CallNode,
            NodeKind::CallOrWriteNode,
            NodeKind::CallAndWriteNode,
            NodeKind::CallOperatorWriteNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node();
        let (receiver, span) = if let Some(call) = &call {
            if call.is_safe_navigation() || call.name().as_slice() != b"monotonic_time" {
                return;
            }
            let Some(receiver) = call.receiver() else { return };
            (receiver, send_span(call))
        } else if let Some((receiver, name, _, span)) = op_assign_send(node, false) {
            if name != b"monotonic_time" {
                return;
            }
            (receiver, span)
        } else {
            return;
        };
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

        let first_argument: Option<Span> = call.as_ref().and_then(|call| {
            call.arguments()
                .and_then(|args| args.arguments().iter().next().map(|a| a.span()))
                .or_else(|| {
                    call.block()
                        .filter(|b| b.kind() == NodeKind::BlockArgumentNode)
                        .map(|b| b.span())
                })
        });
        let optional_unit_parameter = first_argument
            .map(|span| format!(", {}", String::from_utf8_lossy(ctx.text(span))))
            .unwrap_or_default();
        let prefer = format!("Process.clock_gettime(Process::CLOCK_MONOTONIC{optional_unit_parameter})");
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
