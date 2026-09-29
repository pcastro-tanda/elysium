//! `Style/SingleArgumentDig`, ported from RuboCop's
//! `lib/rubocop/cop/style/single_argument_dig.rb` plus the `DigHelp` mixin
//! it includes (`lib/rubocop/cop/mixin/dig_help.rb`).
//!
//! Prism has no parent pointers, so `ignore_dig_chain?`'s `dig?(node.parent)`
//! check needs a lookup built up front: [`SingleArgumentDig::file_start`]
//! walks the whole tree once and records the span of every `CallNode` that
//! satisfies `dig?` (a `.dig(...)` call with at least one argument, none of
//! them a hash). `Context::parent` then gives the immediate parent's span,
//! which is looked up in that set. `dig?(node.receiver)` needs no such
//! lookup since the receiver is a direct child, reachable through the
//! normal accessor.
//!
//! whitequark's `block_pass` argument (`dig(&blk)`) is a member of `send`'s
//! argument list; Prism instead threads it through `CallNode::block`, so it
//! never appears in `arguments()` and needs no explicit exclusion.

use std::collections::HashSet;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks for the use of a `dig` method with a single argument.
#[derive(Debug, Clone)]
pub struct SingleArgumentDig {
    dig_chain_enabled: bool,
    dig_calls: HashSet<Span>,
    /// RuboCop's `ignore_node`/`part_of_ignored_node?`: spans of `dig` calls
    /// already reported, in traversal (outer-to-inner) order. A nested `dig`
    /// call still gets its own offense, but its correction is dropped since
    /// it falls inside a span an enclosing correction already rewrites.
    ignored_spans: Vec<Span>,
}

impl Rule for SingleArgumentDig {
    const META: RuleMeta = RuleMeta {
        name: "Style/SingleArgumentDig",
        department: Department::Style,
        summary: "Avoid using single argument dig method.",
        explanation: "Sometimes using `dig` method ends up with just a single\nargument. In such cases, dig should be replaced with `[]`.\n\nSince replacing `hash&.dig(:key)` with `hash[:key]` could potentially lead to error,\ncalls to the `dig` method using safe navigation will be ignored.\n\n# Safety\n\nThis cop is unsafe because it cannot be guaranteed that the receiver\nis an `Enumerable` or does not have a nonstandard implementation\nof `dig`.\n\n# Examples\n\n```ruby\n# bad\n{ key: 'value' }.dig(:key)\n[1, 2, 3].dig(0)\n\n# good\n{ key: 'value' }[:key]\n[1, 2, 3][0]\n\n# good\n{ key1: { key2: 'value' } }.dig(:key1, :key2)\n[1, [2, [3]]].dig(1, 1)\n\n# good\nkeys = %i[key1 key2]\n{ key1: { key2: 'value' } }.dig(*keys)\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let dig_chain_enabled = options
            .peer("Style/DigChain", "Enabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(false);
        Ok(Self { dig_chain_enabled, dig_calls: HashSet::new(), ignored_spans: Vec::new() })
    }
    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.dig_calls.clear();
        self.ignored_spans.clear();
        let root = ctx.parsed().root();
        collect_dig_calls(&root, &mut self.dig_calls);
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        if call.is_safe_navigation() || call.name().as_slice() != b"dig" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(argument) = single_argument(&call) else { return };
        if self.ignore_dig_chain(&receiver, ctx) {
            return;
        }

        let receiver_text = String::from_utf8_lossy(ctx.text(receiver.span())).into_owned();
        let argument_text = String::from_utf8_lossy(ctx.text(argument.span())).into_owned();
        let original_text = String::from_utf8_lossy(ctx.text(node.span())).into_owned();
        let message =
            format!("Use `{receiver_text}[{argument_text}]` instead of `{original_text}`.");

        let span = node.span();
        let part_of_ignored =
            self.ignored_spans.iter().any(|s| s.start <= span.start && span.end <= s.end);
        if part_of_ignored {
            ctx.report(&Self::META, span, message);
        } else {
            let fix = Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(
                    span,
                    format!("{receiver_text}[{argument_text}]").into_bytes(),
                )],
            };
            ctx.report_with_fix(&Self::META, span, message, fix);
        }
        self.ignored_spans.push(span);
    }
}

impl SingleArgumentDig {
    /// RuboCop's `ignore_dig_chain?`.
    fn ignore_dig_chain(&self, receiver: &Node<'_>, ctx: &Context<'_>) -> bool {
        if !self.dig_chain_enabled {
            return false;
        }
        let receiver_is_dig = self.dig_calls.contains(&receiver.span());
        let parent_is_dig = ctx.parent().is_some_and(|parent| {
            parent.kind == NodeKind::CallNode && self.dig_calls.contains(&parent.span)
        });
        receiver_is_dig || parent_is_dig
    }
}

/// RuboCop's `single_argument_dig?`: a `.dig` call (receiver already
/// verified present by the caller) with exactly one argument that isn't a
/// splat, a forwarded-args placeholder (`...`), or a hash (explicit
/// `{ ... }` or braceless keyword arguments, including anonymous `**`
/// forwarding, which parses as one).
fn single_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let args = call.arguments()?;
    let items: Vec<Node<'pr>> = args.arguments().iter().collect();
    let [argument] = items.as_slice() else { return None };
    match argument.kind() {
        NodeKind::SplatNode
        | NodeKind::ForwardingArgumentsNode
        | NodeKind::HashNode
        | NodeKind::KeywordHashNode => None,
        _ => Some(*argument),
    }
}

/// RuboCop's `dig?`: a call (send or safe-nav) with a receiver, named
/// `dig`, with at least one argument, none of which is a hash.
fn is_dig_call(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.receiver().is_none() {
        return false;
    }
    if call.name().as_slice() != b"dig" {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let items: Vec<Node<'_>> = args.arguments().iter().collect();
    if items.is_empty() {
        return false;
    }
    !items.iter().any(|a| matches!(a.kind(), NodeKind::HashNode | NodeKind::KeywordHashNode))
}

fn collect_dig_calls(node: &Node<'_>, out: &mut HashSet<Span>) {
    if is_dig_call(node) {
        out.insert(node.span());
    }
    ruby_ast::for_each_child(node, |child| collect_dig_calls(child, out));
}
