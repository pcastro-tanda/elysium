//! `Rails/RedundantReceiverInWithOptions`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/redundant_receiver_in_with_options.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Redundant receiver in `with_options`.";

/// Checks for redundant receiver in `with_options`.
#[derive(Debug, Clone)]
pub struct RedundantReceiverInWithOptions;

/// What a `send` node in the block body looks like to `redundant_receiver?`.
struct SendInfo {
    /// `None` for a receiver-less call.
    receiver: Option<ReceiverInfo>,
    dot: Option<Span>,
}

struct ReceiverInfo {
    span: Span,
    /// `lvar` name, or a `sym` value (`children[0]` of the node).
    name: Option<Vec<u8>>,
    is_lvar: bool,
    source: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Block,
    NumBlock,
    ItBlock,
}

impl Rule for RedundantReceiverInWithOptions {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RedundantReceiverInWithOptions",
        department: Department::Rails,
        summary: "Checks for redundant receiver in `with_options`.",
        explanation: "Checks for redundant receiver in `with_options`. Receiver is implicit from \
                      Rails 4.2 or higher.\n\n```ruby\n# bad\nclass Account < ApplicationRecord\n  \
                      with_options dependent: :destroy do |assoc|\n    assoc.has_many :customers\n  \
                      end\nend\n\n# good\nclass Account < ApplicationRecord\n  with_options \
                      dependent: :destroy do\n    has_many :customers\n  end\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"with_options" {
            return;
        }
        let Some(block_node) = call.block() else { return };
        let Some(block) = block_node.as_block_node() else { return };
        let Some(body) = block.body() else { return };

        // `all_block_nodes_in(body).none?`, and `all_send_nodes_in(body)`:
        // whitequark's `send` also wraps the call of an op-assign
        // (`a.b ||= 1`, `a[i] += 1`) and of a multiple-assignment target, but
        // not the `=~` of a `match_with_lvasgn`.
        let mut has_block = false;
        let mut sends: Vec<SendInfo> = Vec::new();
        let mut match_write_call: Option<Span> = None;
        let mut visit = |n: &Node<'_>| {
            let mut push = |safe_navigation: bool,
                            receiver: Option<Node<'_>>,
                            dot: Option<Span>| {
                if !safe_navigation {
                    sends
                        .push(SendInfo { receiver: receiver.map(|r| receiver_info(ctx, &r)), dot });
                }
            };
            let dot = |loc: Option<ruby_ast::Location<'_>>| loc.map(|l| l.span());
            if let Some(b) = n.as_block_node() {
                if block_kind(b.parameters().as_ref()) == BlockKind::Block {
                    has_block = true;
                }
            } else if let Some(l) = n.as_lambda_node() {
                if block_kind(l.parameters().as_ref()) == BlockKind::Block {
                    has_block = true;
                }
            } else if let Some(m) = n.as_match_write_node() {
                match_write_call = Some(m.call().as_node().span());
            } else if let Some(c) = n.as_call_node() {
                if match_write_call != Some(c.as_node().span()) {
                    push(c.is_safe_navigation(), c.receiver(), dot(c.call_operator_loc()));
                }
            } else if let Some(c) = n.as_call_or_write_node() {
                push(c.is_safe_navigation(), c.receiver(), dot(c.call_operator_loc()));
            } else if let Some(c) = n.as_call_and_write_node() {
                push(c.is_safe_navigation(), c.receiver(), dot(c.call_operator_loc()));
            } else if let Some(c) = n.as_call_operator_write_node() {
                push(c.is_safe_navigation(), c.receiver(), dot(c.call_operator_loc()));
            } else if let Some(c) = n.as_index_or_write_node() {
                push(c.is_safe_navigation(), c.receiver(), dot(c.call_operator_loc()));
            } else if let Some(c) = n.as_index_and_write_node() {
                push(c.is_safe_navigation(), c.receiver(), dot(c.call_operator_loc()));
            } else if let Some(c) = n.as_index_operator_write_node() {
                push(c.is_safe_navigation(), c.receiver(), dot(c.call_operator_loc()));
            } else if let Some(c) = n.as_call_target_node() {
                push(
                    c.is_safe_navigation(),
                    Some(c.receiver()),
                    Some(c.call_operator_loc().span()),
                );
            } else if let Some(c) = n.as_index_target_node() {
                push(c.is_safe_navigation(), Some(c.receiver()), None);
            }
        };
        visit(&body);
        each_descendant(&body, &mut visit);
        if has_block {
            return;
        }

        let params = block.parameters();
        let kind = block_kind(params.as_ref());
        if !redundant_receiver(&sends, kind, params.as_ref()) {
            return;
        }

        let mut argument_range = if kind == BlockKind::Block {
            params.as_ref().map(|p| {
                let span = p.span();
                let before = ctx.text(Span::new(0, span.start));
                let spaces = before.iter().rev().take_while(|&&b| b == b' ').count();
                Span::new(span.start - u32::try_from(spaces).expect("offset exceeds u32"), span.end)
            })
        } else {
            None
        };

        for send in &sends {
            let Some(receiver) = &send.receiver else { continue };
            let mut edits = vec![Edit::delete(receiver.span)];
            if let Some(dot) = send.dot {
                edits.push(Edit::delete(dot));
            }
            // Every offense removes the same block argument; overlapping fixes
            // would be rejected, so only the first one carries it.
            if let Some(range) = argument_range.take() {
                edits.push(Edit::delete(range));
            }
            ctx.report_with_fix(
                &Self::META,
                receiver.span,
                MSG,
                Fix { applicability: Applicability::Safe, edits },
            );
        }
    }
}

fn block_kind(parameters: Option<&Node<'_>>) -> BlockKind {
    match parameters.map(Node::kind) {
        Some(NodeKind::NumberedParametersNode) => BlockKind::NumBlock,
        Some(NodeKind::ItParametersNode) => BlockKind::ItBlock,
        _ => BlockKind::Block,
    }
}

fn receiver_info(ctx: &Context<'_>, node: &Node<'_>) -> ReceiverInfo {
    let name = if let Some(lvar) = node.as_local_variable_read_node() {
        Some(lvar.name().as_slice().to_vec())
    } else {
        node.as_symbol_node().map(|s| s.unescaped().to_vec())
    };
    ReceiverInfo {
        span: node.span(),
        name,
        is_lvar: matches!(
            node.kind(),
            NodeKind::LocalVariableReadNode | NodeKind::ItLocalVariableReadNode
        ),
        source: ctx.text(node.span()).to_vec(),
    }
}

/// `redundant_receiver?`.
fn redundant_receiver(sends: &[SendInfo], kind: BlockKind, params: Option<&Node<'_>>) -> bool {
    match kind {
        BlockKind::NumBlock => sends
            .iter()
            .all(|s| s.receiver.as_ref().is_some_and(|r| r.is_lvar && r.source == b"_1")),
        BlockKind::ItBlock => sends
            .iter()
            .all(|s| s.receiver.as_ref().is_some_and(|r| r.is_lvar && r.source == b"it")),
        BlockKind::Block => {
            let Some(arg) = params.and_then(first_argument_name) else { return false };
            sends.iter().all(|s| {
                s.receiver.as_ref().is_some_and(|r| r.name.as_deref() == Some(arg.as_slice()))
            })
        }
    }
}

/// `node.first_argument` name (`children[0]`); `None` when there are no
/// arguments or the first one is unnamed.
fn first_argument_name(params: &Node<'_>) -> Option<Vec<u8>> {
    let parameters = params.as_block_parameters_node()?.parameters()?;
    let mut candidates: Vec<(u32, Option<Vec<u8>>)> = Vec::new();
    let mut push = |n: &Node<'_>| {
        let name = if let Some(p) = n.as_required_parameter_node() {
            Some(p.name().as_slice().to_vec())
        } else if let Some(p) = n.as_optional_parameter_node() {
            Some(p.name().as_slice().to_vec())
        } else if let Some(p) = n.as_rest_parameter_node() {
            p.name().map(|c| c.as_slice().to_vec())
        } else if let Some(p) = n.as_required_keyword_parameter_node() {
            Some(p.name().as_slice().to_vec())
        } else if let Some(p) = n.as_optional_keyword_parameter_node() {
            Some(p.name().as_slice().to_vec())
        } else if let Some(p) = n.as_keyword_rest_parameter_node() {
            p.name().map(|c| c.as_slice().to_vec())
        } else {
            None
        };
        candidates.push((n.span().start, name));
    };
    for n in &parameters.requireds() {
        push(&n);
    }
    for n in &parameters.optionals() {
        push(&n);
    }
    if let Some(n) = parameters.rest() {
        push(&n);
    }
    for n in &parameters.posts() {
        push(&n);
    }
    for n in &parameters.keywords() {
        push(&n);
    }
    if let Some(n) = parameters.keyword_rest() {
        push(&n);
    }
    if let Some(b) = parameters.block() {
        candidates.push((b.as_node().span().start, b.name().map(|c| c.as_slice().to_vec())));
    }
    candidates.into_iter().min_by_key(|(start, _)| *start).and_then(|(_, name)| name)
}
