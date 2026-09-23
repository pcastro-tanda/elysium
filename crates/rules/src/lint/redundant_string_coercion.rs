//! `Lint/RedundantStringCoercion`, ported from RuboCop's
//! `lib/rubocop/cop/lint/redundant_string_coercion.rb` plus the `Interpolation`
//! mixin (`lib/rubocop/cop/mixin/interpolation.rb`) it includes.
//!
//! # Two independent entry points
//!
//! RuboCop's `Interpolation` mixin fires `on_interpolation` for every `begin`
//! node inside a `dstr`/`xstr`/`dsym`/`regexp` -- i.e. every `#{...}` part,
//! regardless of which of those four string-like literals contains it.
//! Prism represents every `#{...}` the same way, as a single
//! [`NodeKind::EmbeddedStatementsNode`], with no distinct node per
//! containing literal kind, so subscribing to that one kind (rather than to
//! each `Interpolated*Node` parent and re-deriving the embedded part) covers
//! all four cases directly.
//!
//! `on_send`, restricted to `print`/`puts`/`warn`, is unrelated to
//! interpolation and is handled separately on every receiverless call to one
//! of those three methods, inspecting its direct argument list.
//!
//! Both paths funnel into the same `to_s_without_args` check (RuboCop's
//! `to_s_without_args?` node-pattern matcher, `(call _ :to_s)`): a `to_s`
//! call -- receiver optional, safe-navigation allowed -- with no positional
//! arguments and no block/block-pass, matching the whitequark node-pattern
//! arity check (a `send`/`csend` node with exactly two children, receiver
//! and method name) that Prism doesn't otherwise enforce structurally.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG_DEFAULT`.
const MSG_DEFAULT: &str = "Redundant use of `Object#to_s` in {}.";
/// RuboCop's `MSG_SELF`.
const MSG_SELF: &str = "Use `self` instead of `Object#to_s` in {}.";
/// RuboCop's `RESTRICT_ON_SEND`.
const RESTRICT_ON_SEND: &[&[u8]] = &[b"print", b"puts", b"warn"];

/// Checks for string conversion in string interpolation, `print`, `puts`,
/// and `warn` arguments, which is redundant.
#[derive(Debug, Clone)]
pub struct RedundantStringCoercion;

impl Rule for RedundantStringCoercion {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RedundantStringCoercion",
        department: Department::Lint,
        summary: "Checks for `Object#to_s` usage in string interpolation.",
        explanation: "\
Checks for string conversion in string interpolation, `print`, `puts`, and
`warn` arguments, which is redundant.

```ruby
# bad
\"result is #{something.to_s}\"
print something.to_s
puts something.to_s
warn something.to_s

# good
\"result is #{something}\"
print something
puts something
warn something
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::EmbeddedStatementsNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::EmbeddedStatementsNode { .. } => {
                let embedded = node.as_embedded_statements_node().expect("kind matched");
                let Some(final_node) = embedded.statements().and_then(|s| s.body().last()) else {
                    return;
                };
                let Node::CallNode { .. } = &final_node else { return };
                let call = final_node.as_call_node().expect("kind matched");
                if to_s_without_args(&call) {
                    register_offense(ctx, &call, "interpolation");
                }
            }
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                if call.receiver().is_some() {
                    return;
                }
                if !RESTRICT_ON_SEND.contains(&call.name().as_slice()) {
                    return;
                }
                let Some(arguments) = call.arguments() else { return };
                for arg in &arguments.arguments() {
                    let Node::CallNode { .. } = &arg else { continue };
                    let child = arg.as_call_node().expect("kind matched");
                    if to_s_without_args(&child) {
                        let context =
                            format!("`{}`", String::from_utf8_lossy(call.name().as_slice()));
                        register_offense(ctx, &child, &context);
                    }
                }
            }
            _ => {}
        }
    }
}

/// RuboCop's `to_s_without_args?` node-pattern matcher: `(call _ :to_s)`.
/// The pattern's fixed arity (receiver, method name, nothing else) means a
/// `to_s` node with any positional argument, keyword argument, block, or
/// block-pass fails to match -- so both are excluded here, not just
/// positional arguments.
fn to_s_without_args(call: &CallNode<'_>) -> bool {
    call.name().as_slice() == b"to_s" && call.arguments().is_none() && call.block().is_none()
}

/// RuboCop's `register_offense`.
fn register_offense(ctx: &mut Context<'_>, call: &CallNode<'_>, context: &str) {
    let selector = call.message_loc().unwrap_or_else(|| call.location());
    let receiver = call.receiver();
    let template = if receiver.is_some() { MSG_DEFAULT } else { MSG_SELF };
    let message = template.replacen("{}", context, 1);

    let replacement: Box<[u8]> = match &receiver {
        Some(recv) => ctx.text(recv.span()).into(),
        None => (*b"self").into(),
    };
    let fix = Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::replace(call.location().span(), replacement)],
    };
    ctx.report_with_fix(&RedundantStringCoercion::META, selector.span(), message, fix);
}
