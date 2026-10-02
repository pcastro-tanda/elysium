//! `Style/HashFetchChain`, ported from RuboCop's
//! `lib/rubocop/cop/style/hash_fetch_chain.rb`.
//!
//! Upstream's `ignore_node`/`ignored_node?` only ever mark whole `fetch`
//! `CallNode`s that [`inspect_chain`] itself walked onto (identity-based, no
//! containment check needed since each is independently re-visited by its
//! own later `enter` call); `ignored` reimplements that as a plain set of
//! exact spans.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext::const_name, node::CallNode, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use std::collections::HashSet;

/// `Hash.new` (bare, no arguments, non-safe-navigation).
fn is_hash_new(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| {
        !call.is_safe_navigation()
            && call.name().as_slice() == b"new"
            && call.arguments().is_none()
            && call.receiver().is_some_and(|r| const_name(&r).as_deref() == Some("Hash"))
    })
}

/// Upstream's `diggable?`: a `.fetch(KEY, DEFAULT)` call (`call`, so safe
/// navigation included) whose `DEFAULT` is `nil`, an empty hash literal, or
/// `Hash.new`. Returns the `KEY` argument.
fn diggable<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"fetch" {
        return None;
    }
    let args: Vec<Node<'pr>> = call.arguments()?.arguments().iter().collect();
    if args.len() != 2 {
        return None;
    }
    let default = &args[1];
    let ok = default.as_nil_node().is_some()
        || default.as_hash_node().is_some_and(|h| h.elements().is_empty())
        || is_hash_new(default);
    ok.then_some(args[0])
}

/// Upstream's `last_fetch_non_nil?`.
fn last_fetch_non_nil(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() != b"fetch" {
        return false;
    }
    let last_arg = call.arguments().and_then(|a| a.arguments().iter().last());
    last_arg.is_none_or(|a| a.as_nil_node().is_none())
}

/// Upstream's `inspect_chain`: walks from `node` inward through its
/// receiver chain for as long as each link is [`diggable`], collecting the
/// key arguments in source order, the innermost (first) link reached
/// (upstream's `last_replaceable_node`, confusingly named from the
/// outside-in walk's point of view), and every visited link's own span
/// (upstream's unconditional `ignore_node` inside the `while` loop).
fn inspect_chain(mut node: Node<'_>) -> (Option<Node<'_>>, Vec<Node<'_>>, Vec<Span>) {
    let mut arguments = Vec::new();
    let mut visited = Vec::new();
    let mut last_replaceable = None;
    while let Some(key) = diggable(&node) {
        arguments.insert(0, key);
        visited.push(node.span());
        last_replaceable = Some(node);
        let call = node.as_call_node().expect("diggable matched a CallNode");
        match call.receiver() {
            Some(receiver) => node = receiver,
            None => break,
        }
    }
    (last_replaceable, arguments, visited)
}

/// Use `Hash#dig` instead of chaining potentially null `fetch` calls.
#[derive(Debug, Clone)]
pub struct HashFetchChain {
    target_ruby_version: f32,
    /// See the module doc.
    ignored: HashSet<Span>,
}

impl Rule for HashFetchChain {
    const META: RuleMeta = RuleMeta {
        name: "Style/HashFetchChain",
        department: Department::Style,
        summary: "Use `Hash#dig` instead of chained `fetch` calls.",
        explanation: "\
Use `Hash#dig` instead of chaining potentially null `fetch` calls.

When `fetch(identifier, nil)` calls are chained on a hash, the expectation
is that each step in the chain returns either `nil` or another hash,
and in both cases, these can be simplified with a single call to `dig` with
multiple arguments.

If the 2nd parameter is `{}` or `Hash.new`, an offense will also be registered,
as long as the final call in the chain is a nil value. If a non-nil value is given,
the chain will not be registered as an offense, as the default value cannot be safely
given with `dig`.

@safety
This cop is unsafe because it cannot be guaranteed that the receiver
is a `Hash` or that `fetch` or `dig` have the expected standard implementation.

```ruby
# bad
hash.fetch('foo', nil)&.fetch('bar', nil)

# bad
# earlier members of the chain can return `{}` as long as the final `fetch`
# has `nil` as a default value
hash.fetch('foo', {}).fetch('bar', nil)

# good
hash.dig('foo', 'bar')

# ok - not handled by the cop since the final `fetch` value is non-nil
hash.fetch('foo', {}).fetch('bar', {})
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version(), ignored: HashSet::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.target_ruby_version < 2.3 {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"fetch" {
            return;
        }
        let span = node.span();
        if self.ignored.contains(&span) || last_fetch_non_nil(&call) {
            return;
        }

        let (last_replaceable, arguments, visited) = inspect_chain(*node);
        self.ignored.extend(visited);
        let Some(last_replaceable) = last_replaceable else { return };
        if arguments.len() <= 1 {
            return;
        }

        let selector_start = last_replaceable
            .as_call_node()
            .and_then(|c| c.message_loc())
            .expect("a diggable `fetch` call always has a selector")
            .span()
            .start;
        let range = Span::new(selector_start, span.end);
        let values = arguments
            .iter()
            .map(|a| String::from_utf8_lossy(ctx.text(a.span())).into_owned())
            .collect::<Vec<_>>()
            .join(", ");
        let replacement = format!("dig({values})");
        let message = format!("Use `{replacement}` instead.");

        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, replacement.into_bytes())],
            },
        );
    }
}
