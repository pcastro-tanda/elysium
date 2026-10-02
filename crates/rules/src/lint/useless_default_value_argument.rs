//! `Lint/UselessDefaultValueArgument`, ported from RuboCop's
//! `lib/rubocop/cop/lint/useless_default_value_argument.rb`.
//!
//! Upstream's `def_node_matcher` fires `on_send`/`on_csend` for a bare
//! `fetch`/`new` call and then separately matches its *parent* against
//! `(any_block {(call !nil? :fetch $_key $_default_value) (send (const _
//! :Array) :new $_size $_default_value)} _args _block_body)`. Prism instead
//! attaches a literal block to its owning [`ruby_ast::node::CallNode`]
//! directly via `call.block()`, so this rule dispatches on
//! [`NodeKind::CallNode`] and reads the block straight off the call being
//! matched, with no parent lookup needed.
//!
//! `(const _ :Array)` matches any constant reference, qualified or not,
//! whose own (right-most) name is `Array`; [`is_array_new_receiver`]
//! mirrors that without requiring a fully-qualified-name match.
//!
//! `hash_without_braces?` (`node.hash_type? && !node.braces?`) distinguishes
//! upstream's single `hash` node type by its `braces?` node-matcher helper;
//! Prism instead gives a braced hash (`{}`) and a braceless keyword hash
//! (`default: value`, `**kwarg`) distinct node kinds
//! ([`NodeKind::HashNode`] vs [`NodeKind::KeywordHashNode`]), so the check
//! collapses to a kind comparison.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Block supersedes default value argument.";

/// `(const _ :Array)`: any constant reference (qualified or not) whose
/// right-most segment is named `Array`.
fn is_array_new_receiver(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"Array")
        }
        NodeKind::ConstantPathNode => node
            .as_constant_path_node()
            .and_then(|p| p.name())
            .is_some_and(|name| name.as_slice() == b"Array"),
        _ => false,
    }
}

/// `AllowedReceivers#receiver_name`. Only a `CallNode` ever has a receiver
/// at all; every other node kind falls straight through to `receiver.source`.
fn receiver_name(node: &Node<'_>, ctx: &Context<'_>) -> String {
    let Some(call) = node.as_call_node() else {
        return String::from_utf8_lossy(ctx.text(node.span())).into_owned();
    };
    if let Some(recv) = call.receiver() {
        let is_const =
            matches!(recv.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode);
        if !is_const {
            return receiver_name(&recv, ctx);
        }
    }
    if call.is_safe_navigation() {
        return String::from_utf8_lossy(ctx.text(node.span())).into_owned();
    }
    match call.receiver() {
        Some(recv) => {
            format!(
                "{}.{}",
                receiver_name(&recv, ctx),
                String::from_utf8_lossy(call.name().as_slice())
            )
        }
        None => String::from_utf8_lossy(call.name().as_slice()).into_owned(),
    }
}

/// Checks for usage of `fetch` or `Array.new` with default value argument and block.
#[derive(Debug, Clone)]
pub struct UselessDefaultValueArgument {
    /// `AllowedReceivers`: receiver names ([`receiver_name`]) never flagged.
    allowed_receivers: Vec<String>,
}

impl UselessDefaultValueArgument {
    fn is_allowed_receiver(&self, node: &Node<'_>, ctx: &Context<'_>) -> bool {
        let name = receiver_name(node, ctx);
        self.allowed_receivers.contains(&name)
    }
}

impl Rule for UselessDefaultValueArgument {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UselessDefaultValueArgument",
        department: Department::Lint,
        summary:
            "Checks for usage of `fetch` or `Array.new` with default value argument and block.",
        explanation: "\
Checks for usage of method `fetch` or `Array.new` with default value argument and block. \
In such cases, block will always be used as default value.

This cop emulates Ruby warning \"block supersedes default value argument\" which applies to \
`Array.new`, `Array#fetch`, `Hash#fetch`, `ENV.fetch` and `Thread#fetch`.

A `fetch` call without a receiver is considered a custom method and does not register an \
offense.

```ruby
# bad
x.fetch(key, default_value) { block_value }
Array.new(size, default_value) { block_value }

# good
x.fetch(key) { block_value }
Array.new(size) { block_value }

# also good - in case default value argument is desired instead
x.fetch(key, default_value)
Array.new(size, default_value)

# good - keyword arguments aren't registered as offenses
x.fetch(key, keyword: :arg) { block_value }
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "AllowedReceivers",
            default: ConfigDefault::StrList(&[]),
            allowed: &[],
            doc: "Receiver names (`AllowedReceivers#receiver_name`) never flagged.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allowed_receivers: options.str_list("AllowedReceivers") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        let Some(receiver) = call.receiver() else { return };
        let name = call.name().as_slice();
        let is_fetch = name == b"fetch";
        let is_array_new = name == b"new" && is_array_new_receiver(&receiver);
        if !is_fetch && !is_array_new {
            return;
        }

        let Some(block_field) = call.block() else { return };
        if block_field.as_block_node().is_none() {
            return;
        }

        let Some(arguments) = call.arguments() else { return };
        let args = arguments.arguments();
        if args.len() != 2 {
            return;
        }
        let prev_arg = args.iter().next().expect("checked len");
        let default_value = args.iter().nth(1).expect("checked len");

        if self.is_allowed_receiver(&receiver, ctx) {
            return;
        }
        if default_value.kind() == NodeKind::KeywordHashNode {
            return;
        }

        let span = default_value.span();
        let edit_span = Span::new(prev_arg.span().end, span.end);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix { applicability: Applicability::Unsafe, edits: vec![Edit::delete(edit_span)] },
        );
    }
}
