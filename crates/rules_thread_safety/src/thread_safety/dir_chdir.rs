//! `ThreadSafety/DirChdir`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/dir_chdir.rb`.

use std::collections::HashSet;

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, is_bare_or_toplevel_const};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

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

/// Avoid using `Dir.chdir` due to its process-wide effect.
#[derive(Debug, Clone)]
pub struct DirChdir {
    allow_call_with_block: bool,
    /// Spans of calls that are the sole statement of a (non-numbered) block
    /// body: whitequark gives such a call the block as its direct parent.
    block_body_calls: HashSet<(u32, u32)>,
}

impl Rule for DirChdir {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/DirChdir",
        department: Department::ThreadSafety,
        summary: "Avoid using `Dir.chdir` due to its process-wide effect.",
        explanation: "Avoid using `Dir.chdir` due to its process-wide effect. If `AllowCallWithBlock` (disabled by default) option is enabled, calling `Dir.chdir` with block will be allowed.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::CallOrWriteNode,
            NodeKind::CallAndWriteNode,
            NodeKind::CallOperatorWriteNode,
            NodeKind::BlockNode,
            NodeKind::LambdaNode,
        ],
        config: &[ConfigOption {
            name: "AllowCallWithBlock",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Allow `Dir.chdir` and friends when called with a block.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_call_with_block: options.bool("AllowCallWithBlock"),
            block_body_calls: HashSet::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::BlockNode => {
                if let Some(block) = node.as_block_node() {
                    self.note_block_body(block.parameters(), block.body());
                }
            }
            NodeKind::LambdaNode => {
                if let Some(lambda) = node.as_lambda_node() {
                    self.note_block_body(lambda.parameters(), lambda.body());
                }
            }
            NodeKind::CallNode
            | NodeKind::CallOrWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::CallOperatorWriteNode => self.on_send(node, ctx),
            _ => {}
        }
    }
}

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

impl DirChdir {
    fn note_block_body(&mut self, parameters: Option<Node<'_>>, body: Option<Node<'_>>) {
        if parameters.is_some_and(|p| {
            matches!(p.kind(), NodeKind::NumberedParametersNode | NodeKind::ItParametersNode)
        }) {
            return;
        }
        let Some(statements) = body.as_ref().and_then(Node::as_statements_node) else { return };
        let mut it = statements.body().iter();
        if let (Some(only), None) = (it.next(), it.next()) {
            if only.kind() == NodeKind::CallNode {
                let span = only.span();
                self.block_body_calls.insert((span.start, span.end));
            }
        }
    }

    fn on_send(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node();
        let (receiver, name, dot_span, span) = if let Some(call) = &call {
            let Some(receiver) = call.receiver() else { return };
            (
                receiver,
                call.name().as_slice().to_vec(),
                call.call_operator_loc().map(|l| l.span()),
                send_span(call),
            )
        } else if let Some(parts) = op_assign_send(node, true) {
            (parts.0, parts.1, parts.2, parts.3)
        } else {
            return;
        };
        let name = name.as_slice();
        if name != b"chdir" && name != b"cd" {
            return;
        }
        if !is_bare_or_toplevel_const(&receiver) {
            return;
        }
        let module: &[u8] = if let Some(c) = receiver.as_constant_read_node() {
            c.name().as_slice()
        } else if let Some(p) = receiver.as_constant_path_node() {
            let Some(n) = p.name() else { return };
            n.as_slice()
        } else {
            return;
        };
        let matched = match name {
            b"chdir" => module == b"Dir" || module == b"FileUtils",
            _ => module == b"FileUtils",
        };
        if !matched {
            return;
        }

        if let (true, Some(call)) = (self.allow_call_with_block, &call) {
            let block_argument = call
                .block()
                .as_ref()
                .is_some_and(|b| b.kind() == NodeKind::BlockArgumentNode);
            let own_block = call.block().is_some_and(|b| {
                b.as_block_node().is_some_and(|block| {
                    !block.parameters().is_some_and(|p| {
                        matches!(
                            p.kind(),
                            NodeKind::NumberedParametersNode | NodeKind::ItParametersNode
                        )
                    })
                })
            });
            let node_span = node.span();
            if block_argument
                || own_block
                || self.block_body_calls.contains(&(node_span.start, node_span.end))
            {
                return;
            }
        }

        let dot = dot_span.map_or_else(String::new, |loc| {
            String::from_utf8_lossy(ctx.text(loc)).into_owned()
        });
        let message = format!(
            "Avoid using `{}{dot}{}` due to its process-wide effect.",
            String::from_utf8_lossy(module),
            String::from_utf8_lossy(name),
        );
        ctx.report(&Self::META, span, message);
    }
}
