//! `Security/CompoundHash`, ported from RuboCop's
//! `lib/rubocop/cop/security/compound_hash.rb`.
//!
//! # `contained_in_hash_method?` and `outer_bad_hash_combinator?`
//!
//! Upstream expresses both checks as ancestor walks (`node.each_ancestor`)
//! evaluated lazily from inside `on_send`. Since this port visits every
//! node top-down anyway, both are reproduced as depth counters maintained
//! in `enter`/`leave`: [`CompoundHash::hash_method_depth`] counts enclosing
//! zero-arity `hash`-method definitions (`def hash`/`def self.hash`/
//! `define_method(:hash)`/`define_singleton_method(:hash)`), and
//! [`CompoundHash::combinator_depth`] counts enclosing
//! [`is_bad_hash_combinator`] nodes so only the outermost one in a nest is
//! ever reported.
//!
//! # `redundant_hash?`
//!
//! Upstream's `(^^(send array ... :hash) _ :hash)` pattern, read from the
//! *inner* `.hash` call's own `on_send` dispatch, means: this node's
//! grandparent (its array-literal parent's parent) is itself an
//! array-receiver `.hash` call. Since an array literal's elements are
//! always its immediate children, that is equivalent -- checked forward,
//! from the *outer* `.hash` call -- to: for every direct element of the
//! array, if that element is itself a `.hash` call (any receiver), it is
//! redundant. [`CompoundHash::enter`] implements it that way instead of
//! climbing ancestors, since [`linter::Context::ancestors`] only exposes
//! `NodeInfo` (kind + span), not enough to repeat the shape check
//! backwards.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::{CallNode, DefNode, ParametersNode};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `COMBINATOR_IN_HASH_MSG`.
const COMBINATOR_IN_HASH_MSG: &str = "Use `[...].hash` instead of combining hash values manually.";
/// RuboCop's `MONUPLE_HASH_MSG`.
const MONUPLE_HASH_MSG: &str =
    "Delegate hash directly without wrapping in an array when only using a single value.";
/// RuboCop's `REDUNDANT_HASH_MSG`.
const REDUNDANT_HASH_MSG: &str = "Calling .hash on elements of a hashed array is redundant.";

/// RuboCop's `RESTRICT_ON_SEND` combinator operators (`bad_hash_combinator?`).
const BAD_COMBINATOR_OPS: [&[u8]; 4] = [b"^", b"+", b"*", b"|"];

/// Checks for `hash` implementations that combine values manually instead of
/// delegating to `Array#hash`.
#[derive(Debug, Clone, Default)]
pub struct CompoundHash {
    /// RuboCop's `contained_in_hash_method?`: how many enclosing zero-arity
    /// `hash`-method definitions (static or dynamic) we are currently
    /// inside.
    hash_method_depth: u32,
    /// RuboCop's `outer_bad_hash_combinator?`: how many enclosing
    /// [`is_bad_hash_combinator`] nodes we are currently inside.
    combinator_depth: u32,
}

impl Rule for CompoundHash {
    const META: RuleMeta = RuleMeta {
        name: "Security/CompoundHash",
        department: Department::Security,
        summary: "Checks for `hash` implementations that combine values manually instead of delegating to `Array#hash`.",
        explanation: "\
Checks for implementations of the `hash` method which combine
values using custom logic instead of delegating to `Array#hash`.

Manually combining hashes is error prone and hard to follow, especially
when there are many values. Poor implementations may also introduce
performance or security concerns if they are prone to collisions.
Delegating to `Array#hash` is clearer and safer, although it might be slower
depending on the use case.

```ruby
# bad
def hash
  @foo ^ @bar
end

# good
def hash
  [@foo, @bar].hash
end
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::DefNode,
            NodeKind::CallOperatorWriteNode,
            NodeKind::ClassVariableOperatorWriteNode,
            NodeKind::ConstantOperatorWriteNode,
            NodeKind::ConstantPathOperatorWriteNode,
            NodeKind::GlobalVariableOperatorWriteNode,
            NodeKind::IndexOperatorWriteNode,
            NodeKind::InstanceVariableOperatorWriteNode,
            NodeKind::LocalVariableOperatorWriteNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(def) = node.as_def_node() {
            if is_hash_method_def(&def) {
                self.hash_method_depth += 1;
            }
            return;
        }

        if is_bad_hash_combinator(node) {
            if self.combinator_depth == 0 && self.hash_method_depth > 0 {
                ctx.report(&Self::META, node.span(), COMBINATOR_IN_HASH_MSG);
            }
            self.combinator_depth += 1;
        }

        let Some(call) = node.as_call_node() else { return };

        if is_dynamic_hash_method_def(&call) {
            self.hash_method_depth += 1;
        }

        if call.name().as_slice() != b"hash" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(array) = receiver.as_array_node() else { return };

        if array.elements().len() == 1 {
            ctx.report(&Self::META, node.span(), MONUPLE_HASH_MSG);
        }
        for element in &array.elements() {
            if is_hash_call(&element) {
                ctx.report(&Self::META, element.span(), REDUNDANT_HASH_MSG);
            }
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if let Some(def) = node.as_def_node() {
            if is_hash_method_def(&def) {
                self.hash_method_depth -= 1;
            }
            return;
        }

        if is_bad_hash_combinator(node) {
            self.combinator_depth -= 1;
        }

        if let Some(call) = node.as_call_node() {
            if is_dynamic_hash_method_def(&call) {
                self.hash_method_depth -= 1;
            }
        }
    }
}

/// RuboCop's `bad_hash_combinator?`: `({send | op-asgn} _ {:^ | :+ | :* |
/// :|} _)`. A plain call or an operator-assignment whose operator/method is
/// one of `^`, `+`, `*`, `|`.
fn is_bad_hash_combinator(node: &Node<'_>) -> bool {
    if let Some(call) = node.as_call_node() {
        return BAD_COMBINATOR_OPS.contains(&call.name().as_slice());
    }
    let operator = node
        .as_call_operator_write_node()
        .map(|n| n.binary_operator())
        .or_else(|| node.as_class_variable_operator_write_node().map(|n| n.binary_operator()))
        .or_else(|| node.as_constant_operator_write_node().map(|n| n.binary_operator()))
        .or_else(|| node.as_constant_path_operator_write_node().map(|n| n.binary_operator()))
        .or_else(|| node.as_global_variable_operator_write_node().map(|n| n.binary_operator()))
        .or_else(|| node.as_index_operator_write_node().map(|n| n.binary_operator()))
        .or_else(|| node.as_instance_variable_operator_write_node().map(|n| n.binary_operator()))
        .or_else(|| node.as_local_variable_operator_write_node().map(|n| n.binary_operator()));
    operator.is_some_and(|op| BAD_COMBINATOR_OPS.contains(&op.as_slice()))
}

/// Whether `node` is a `.hash` call (any receiver, any safe-navigation).
fn is_hash_call(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|c| c.name().as_slice() == b"hash")
}

/// RuboCop's `static_hash_method_definition?`: `({def | defs _} :hash (args)
/// _)` -- a zero-arity `hash`/`self.hash` method definition.
fn is_hash_method_def(def: &DefNode<'_>) -> bool {
    def.name().as_slice() == b"hash" && has_zero_params(def.parameters())
}

/// RuboCop's `dynamic_hash_method_definition?`: `(block (send _
/// {:define_method | :define_singleton_method} (sym :hash)) (args) _)` -- a
/// `define_method(:hash)`/`define_singleton_method(:hash)` call with a
/// zero-arity block.
fn is_dynamic_hash_method_def(call: &CallNode<'_>) -> bool {
    let name = call.name().as_slice();
    if name != b"define_method" && name != b"define_singleton_method" {
        return false;
    }
    let Some(arguments) = call.arguments() else { return false };
    if arguments.arguments().len() != 1 {
        return false;
    }
    let Some(first_arg) = arguments.arguments().iter().next() else { return false };
    let Some(symbol) = first_arg.as_symbol_node() else { return false };
    if symbol.unescaped() != b"hash" {
        return false;
    }
    let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return false };
    is_zero_arity_block_params(block.parameters())
}

/// Whether a `def`'s parameters are absent or all empty (whitequark's bare
/// `(args)` with no children).
fn has_zero_params(params: Option<ParametersNode<'_>>) -> bool {
    match params {
        None => true,
        Some(p) => {
            p.requireds().is_empty()
                && p.optionals().is_empty()
                && p.rest().is_none()
                && p.posts().is_empty()
                && p.keywords().is_empty()
                && p.keyword_rest().is_none()
                && p.block().is_none()
        }
    }
}

/// Whether a block's parameters (absent, or a zero-arity `BlockParametersNode`
/// with no locals either) count as whitequark's bare `(args)`.
fn is_zero_arity_block_params(params: Option<Node<'_>>) -> bool {
    match params {
        None => true,
        Some(node) => node
            .as_block_parameters_node()
            .is_some_and(|bp| has_zero_params(bp.parameters()) && bp.locals().is_empty()),
    }
}
