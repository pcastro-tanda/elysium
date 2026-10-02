//! `Style/DigChain`, ported from RuboCop's
//! `lib/rubocop/cop/style/dig_chain.rb`, plus the `DigHelp`/`CommentsHelp`
//! mixins it includes.
//!
//! # `dig?`
//!
//! Upstream's `(call _ :dig !{hash block_pass}+)` requires at least one
//! argument, none of which is a `hash` (an explicit `{...}` literal *or*
//! implicit keyword args -- both the same whitequark type) or a
//! `block_pass`. Prism's equivalents are [`NodeKind::KeywordHashNode`]
//! (covering both the explicit-braces and implicit-kwargs spellings, per
//! the porting kit) and [`NodeKind::BlockArgumentNode`]; an anonymous
//! `**`-forward argument is additionally excluded as
//! [`NodeKind::AssocSplatNode`] in case Prism leaves it unwrapped rather
//! than inside a `KeywordHashNode`.
//!
//! # Chain walking and offense merging
//!
//! Upstream dispatches `on_send` for every `.dig` call (outermost first,
//! since the commissioner visits a node before its receiver) and walks
//! *down* the receiver chain from there, `ignore_node`-ing every consumed
//! ancestor so its own later `on_send` call is a no-op. This port tracks
//! the same thing with `handled`, a per-file `HashSet<Span>` populated the
//! first time (always the outermost member, by the same visit-order
//! argument) a chain is found.
//!
//! # Moved comments
//!
//! A comment physically between two chained `.dig` calls would otherwise
//! be destroyed by replacing the whole chain's span; upstream relocates
//! each one (in source order) to its own line immediately before the
//! chain, via repeated `insert_before` calls in reverse order that the
//! corrector resolves back into forward order. This port instead builds
//! the already-forward-ordered block of moved comment lines directly and
//! inserts it once.

use std::collections::HashSet;

use linter::{
    Applicability, CommentInfo, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG_TEMPLATE: &str = "Use `%<replacement>s` instead of chaining.";

/// Use `dig` with multiple parameters instead of chaining multiple calls.
#[derive(Debug, Clone, Default)]
pub struct DigChain {
    /// Spans of `.dig` calls already folded into an outer chain's offense,
    /// so their own (later) `enter` is a no-op -- upstream's `ignore_node`.
    handled: HashSet<Span>,
}

impl Rule for DigChain {
    const META: RuleMeta = RuleMeta {
        name: "Style/DigChain",
        department: Department::Style,
        summary: "Use `dig` with multiple parameters instead of chaining multiple calls.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.handled.contains(&node.span()) {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.call_operator_loc().is_none() || !is_dig_call(&call) {
            return;
        }

        let mut arguments: Vec<Node<'_>> =
            call.arguments().map_or(Vec::new(), |a| a.arguments().iter().collect());
        let mut begin: Option<ruby_ast::node::Location<'_>> = None;
        let mut current = call.receiver();
        while let Some(recv) = current {
            let Some(recv_call) = recv.as_call_node() else { break };
            if !is_dig_call(&recv_call) {
                break;
            }
            let Some(selector) = recv_call.message_loc() else { break };
            let mut recv_args: Vec<Node<'_>> =
                recv_call.arguments().map_or(Vec::new(), |a| a.arguments().iter().collect());
            recv_args.append(&mut arguments);
            arguments = recv_args;
            begin = Some(selector);
            self.handled.insert(recv.span());
            current = recv_call.receiver();
        }
        let Some(begin) = begin else { return };
        if invalid_arguments(&arguments) {
            return;
        }

        let range = Span::new(begin.span().start, node.span().end);
        let replacement = format!(
            "dig({})",
            arguments
                .iter()
                .map(|a| String::from_utf8_lossy(ctx.text(a.span())).into_owned())
                .collect::<Vec<_>>()
                .join(", ")
        );
        let message = MSG_TEMPLATE.replace("%<replacement>s", &replacement);

        let mut edits = vec![Edit::replace(range, replacement.into_bytes())];
        let moved: Vec<&CommentInfo> = ctx
            .comments()
            .iter()
            .filter(|c| c.span.start >= node.span().start && c.span.start < range.end)
            .collect();
        if !moved.is_empty() {
            let mut text = String::new();
            for comment in &moved {
                text.push_str(&String::from_utf8_lossy(ctx.text(comment.span)));
                text.push('\n');
            }
            edits.push(Edit::insert(node.span().start, text.into_bytes()));
        }

        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// `dig?`: a call named `dig` with at least one argument, none of which is
/// a hash (explicit or implicit keyword args) or a block-pass.
fn is_dig_call(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() != b"dig" {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let args = args.arguments();
    if args.is_empty() {
        return false;
    }
    args.iter().all(|a| {
        !matches!(
            a.kind(),
            NodeKind::KeywordHashNode
                | NodeKind::HashNode
                | NodeKind::AssocSplatNode
                | NodeKind::BlockArgumentNode
        )
    })
}

/// `invalid_arguments?`: a forwarded `...` argument appears anywhere but
/// last in the combined argument list.
fn invalid_arguments(arguments: &[Node<'_>]) -> bool {
    let Some(index) = arguments.iter().position(|a| a.kind() == NodeKind::ForwardingArgumentsNode)
    else {
        return false;
    };
    index < arguments.len() - 1
}
