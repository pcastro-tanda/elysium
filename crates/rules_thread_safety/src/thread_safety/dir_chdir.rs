//! `ThreadSafety/DirChdir`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/dir_chdir.rb`.

use std::collections::HashSet;

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, is_bare_or_toplevel_const};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

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
        kinds: &[NodeKind::CallNode, NodeKind::BlockNode, NodeKind::LambdaNode],
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
            NodeKind::CallNode => self.on_send(node, ctx),
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
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let name = name.as_slice();
        if name != b"chdir" && name != b"cd" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
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

        if self.allow_call_with_block {
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
            let span = node.span();
            if block_argument
                || own_block
                || self.block_body_calls.contains(&(span.start, span.end))
            {
                return;
            }
        }

        let dot = call.call_operator_loc().map_or_else(String::new, |loc| {
            String::from_utf8_lossy(ctx.text(loc.span())).into_owned()
        });
        let message = format!(
            "Avoid using `{}{dot}{}` due to its process-wide effect.",
            String::from_utf8_lossy(module),
            String::from_utf8_lossy(name),
        );
        ctx.report(&Self::META, send_span(&call), message);
    }
}
