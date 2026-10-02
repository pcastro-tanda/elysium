//! `Lint/ItWithoutArgumentsInBlock`, ported from RuboCop's
//! `lib/rubocop/cop/lint/it_without_arguments_in_block.rb`.
//!
//! # Node shapes
//!
//! Prism's grammar is itself version-aware: parsed with a target Ruby
//! version below 3.4 (this port's and the fixtures' default), a bare `it`
//! identifier is an ordinary `CallNode` (`:it`, no receiver, no args) just
//! as it is in whitequark -- `maximum_target_ruby_version 3.3`'s gate and
//! `deprecated_it_method?`'s shape checks therefore port directly. Parsed at
//! 3.4+ (only ever exercised here through an explicit `TargetRubyVersion`
//! override), Prism instead recognizes the same bare `it` as its Ruby 3.4
//! implicit block parameter and emits a dedicated `ItLocalVariableReadNode`
//! kind, with no `CallNode` to match at all -- so the `target_ruby_version`
//! gate below is redundant with the parser's own version-gated grammar for
//! every fixture, but is kept to mirror upstream's explicit guard literally
//! and to stay correct should some future parse path not fork on version.
//!
//! RuboCop's `block_node.arguments.empty_and_without_delimiters?` (no `|...|`
//! at all, not even empty `||`) is Prism's `BlockNode::parameters` reading
//! `None` -- an explicit (even empty) pipe pair instead yields
//! `BlockParametersNode`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "`it` calls without arguments will refer to the first block param in Ruby 3.4; \
use `it()` or `self.it`.";

/// Checks uses of `it` calls without arguments in block.
#[derive(Debug, Clone)]
pub struct ItWithoutArgumentsInBlock {
    target_ruby_version: f32,
    /// Whether each currently-open `BlockNode` has no explicit parameter
    /// list at all (RuboCop's `empty_and_without_delimiters?`).
    block_stack: Vec<bool>,
}

impl Rule for ItWithoutArgumentsInBlock {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ItWithoutArgumentsInBlock",
        department: Department::Lint,
        summary: "Checks uses of `it` calls without arguments in block.",
        explanation: "\
`it` calls without arguments will refer to the first block param in Ruby 3.4.
So use `it()` or `self.it` to ensure compatibility.

```ruby
# bad
do_something { it }

# good
do_something { it() }
do_something { self.it }
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::BlockNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version(), block_stack: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.block_stack.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(block) = node.as_block_node() {
            self.block_stack.push(block.parameters().is_none());
            return;
        }

        let Some(call) = node.as_call_node() else { return };

        if self.target_ruby_version <= 3.3
            && self.block_stack.last().copied() == Some(true)
            && call.name().as_slice() == b"it"
            && call.receiver().is_none()
            && call.arguments().is_none()
            && call.opening_loc().is_none()
            && call.block().is_none()
        {
            ctx.report(&Self::META, node.span(), MSG);
        }

        // Whitequark unifies a call with its own attached literal block
        // into a single `block` node, nesting the call's receiver and
        // arguments as descendants of that wrapping node even though
        // they're written (and evaluated) before the block itself. Prism
        // keeps the block as a separate `CallNode::block()` field, a
        // sibling of `receiver()`/`arguments()` rather than their
        // ancestor, so `each_ancestor(:block)` run against such a
        // receiver/argument would still find this call's own block in
        // whitequark; push its emptiness here too so it's visible for the
        // whole span of this call, not just its block's own body (which
        // pushes its own, redundant but harmless, identical entry below).
        if let Some(own_block) = call.block().and_then(|b| b.as_block_node()) {
            self.block_stack.push(own_block.parameters().is_none());
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.as_block_node().is_some() {
            self.block_stack.pop();
            return;
        }
        if let Some(call) = node.as_call_node() {
            if call.block().and_then(|b| b.as_block_node()).is_some() {
                self.block_stack.pop();
            }
        }
    }
}
