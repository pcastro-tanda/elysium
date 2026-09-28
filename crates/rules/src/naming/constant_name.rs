//! `Naming/ConstantName`, ported from RuboCop's
//! `lib/rubocop/cop/naming/constant_name.rb`.
//!
//! # Node shapes
//!
//! Whitequark's parser gives every constant write the same `casgn` node
//! type regardless of form; upstream's `on_casgn` (plus a check of the
//! node's parent for the `or_asgn` case) covers plain `CONST = value`,
//! `CONST ||= value`, their `A::B::CONST` qualified forms, and each
//! constant target inside a multiple assignment (`A, B = 1, 2` parses as a
//! `masgn` whose `mlhs` holds one `casgn` node per target, each with a
//! `nil` value). Prism splits these into six distinct node kinds instead:
//! [`NodeKind::ConstantWriteNode`]/[`NodeKind::ConstantPathWriteNode`] for
//! the plain forms, [`NodeKind::ConstantOrWriteNode`]/
//! [`NodeKind::ConstantPathOrWriteNode`] for `||=`, and
//! [`NodeKind::ConstantTargetNode`]/[`NodeKind::ConstantPathTargetNode`]
//! for a target inside a `MultiWriteNode`'s `lefts`/`rest`/`rights` -- so
//! subscribing to all six, each handled directly, reproduces every case
//! upstream's single `on_casgn` handles without needing to walk `masgn`
//! ourselves (the engine's traversal visits each target node on its own).
//!
//! A target node has no value to inspect (matching upstream: `node.parent`
//! is a `masgn`/`mlhs`, not an `or_asgn`, so `value` is `nil` and
//! `allowed_assignment?(nil)` is always `false`) -- only its name is
//! checked.
//!
//! # `allowed_assignment?`
//!
//! - `%i[block const casgn].include?(value.type)`: a `block`-type value
//!   (whitequark wraps a method call with a literal block in one `block`
//!   node) is, in Prism, an ordinary [`NodeKind::CallNode`] whose own
//!   `block` field points at a [`NodeKind::BlockNode`] (as opposed to a
//!   `&block` argument pass, which is a distinct `BlockArgumentNode` and
//!   does not count, mirroring `ConstantDefinitionInBlock`'s `any_block_type?`
//!   note). A `const`-type value is a bare or qualified constant reference
//!   ([`NodeKind::ConstantReadNode`]/[`NodeKind::ConstantPathNode`]). A
//!   `casgn`-type value is a nested constant assignment used as a value
//!   (`Bar = Foo = Qux` parses right-associatively in Prism as a
//!   `ConstantWriteNode` whose own `value` is another `ConstantWriteNode`).
//! - `allowed_method_call_on_rhs?`/`literal_receiver?`: a method call is
//!   allowed unless it has an explicit receiver that is itself a literal
//!   (or a parenthesized literal, e.g. `(5).some_method`) -- ported as
//!   [`has_literal_receiver`], covering the same literal-node-kind set
//!   [`crate::style::mutable_constant`] enumerates from RuboCop's
//!   `LITERALS`.
//! - `class_or_struct_return_method?`: `(send (const _ {:Class :Struct})
//!   :new ...)`; the pattern's `_` namespace wildcard matches any
//!   namespace (or none), so only the receiver's own short name is
//!   compared.
//! - `allowed_conditional_expression_on_rhs?`/`contains_constant?`: an
//!   `if`/`unless`/ternary value (all three share Prism's single
//!   [`NodeKind::IfNode`] for `if`/ternary, matching whitequark's own
//!   `:if` type covering `if`, `unless` -- swapped branches -- and
//!   ternary) is allowed when *any* of its branches (recursing through an
//!   `elsif` chain, mirroring rubocop-ast's `IfNode#branches`) is itself a
//!   bare constant read -- not when *all* branches are, despite the
//!   `contains_constant?` name.
//!
//! # `SNAKE_CASE`
//!
//! `/^[[:digit:][:upper:]_]+$/` uses POSIX character classes so that
//! accented uppercase letters count; this port uses Rust's Unicode-aware
//! `char::is_uppercase`, which is not a byte-for-byte match of Oniguruma's
//! POSIX `[:upper:]` table but agrees on every accented Latin letter the
//! upstream doc comment itself calls out (`TÖP_TEST`).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::{CallNode, IfNode, StatementsNode, UnlessNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Use SCREAMING_SNAKE_CASE for constants.";

/// RuboCop's `SNAKE_CASE` regexp, applied to a constant's short name.
fn is_screaming_snake_case(name: &[u8]) -> bool {
    match std::str::from_utf8(name) {
        Ok(s) => {
            !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c.is_uppercase() || c == '_')
        }
        Err(_) => false,
    }
}

/// RuboCop's `LITERALS`, the node kinds `Node#literal?` recognizes.
fn is_literal_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::RangeNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
    )
}

/// A parenthesized node's own single statement, if it holds exactly one
/// (RuboCop's `(begin X)` shape): `(5)` unwraps to `5`, `(5; 6)` (two
/// statements) unwraps to nothing.
fn parenthesized_single_statement<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    if node.kind() != NodeKind::ParenthesesNode {
        return None;
    }
    let mut body = node.as_parentheses_node()?.body();
    if let Some(inner) = &body {
        if inner.kind() == NodeKind::StatementsNode {
            let stmts = inner.as_statements_node()?.body();
            body = if stmts.len() == 1 { stmts.iter().next() } else { None };
        }
    }
    body
}

/// RuboCop's `literal_receiver?`, applied to a call's own receiver: a
/// literal, or a parenthesized literal.
fn has_literal_receiver(call: &CallNode<'_>) -> bool {
    let Some(receiver) = call.receiver() else { return false };
    if is_literal_kind(receiver.kind()) {
        return true;
    }
    parenthesized_single_statement(&receiver).is_some_and(|s| is_literal_kind(s.kind()))
}

/// RuboCop's `allowed_method_call_on_rhs?`.
fn allowed_method_call_on_rhs(value: &Node<'_>) -> bool {
    value.as_call_node().is_some_and(|call| !has_literal_receiver(&call))
}

/// A bare or qualified constant reference's own short name, regardless of
/// namespace (RuboCop's `_` wildcard in `class_or_struct_return_method?`).
fn const_short_name<'pr>(node: &Node<'pr>) -> Option<&'pr [u8]> {
    match node.kind() {
        NodeKind::ConstantReadNode => node.as_constant_read_node().map(|c| c.name().as_slice()),
        NodeKind::ConstantPathNode => node.as_constant_path_node()?.name().map(|n| n.as_slice()),
        _ => None,
    }
}

/// RuboCop's `class_or_struct_return_method?`: `(send (const _ {:Class
/// :Struct}) :new ...)`.
fn is_class_or_struct_new(value: &Node<'_>) -> bool {
    let Some(call) = value.as_call_node() else { return false };
    if call.name().as_slice() != b"new" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    matches!(const_short_name(&receiver), Some(b"Class" | b"Struct"))
}

/// RuboCop's `%i[block const casgn].include?(value.type)`.
fn is_block_const_or_casgn(value: &Node<'_>) -> bool {
    match value.kind() {
        NodeKind::ConstantReadNode
        | NodeKind::ConstantPathNode
        | NodeKind::ConstantWriteNode
        | NodeKind::ConstantPathWriteNode => true,
        NodeKind::CallNode => value
            .as_call_node()
            .and_then(|c| c.block())
            .is_some_and(|b| b.kind() == NodeKind::BlockNode),
        _ => false,
    }
}

/// A branch body's single statement, if it holds exactly one (RuboCop's
/// `IfNode#if_branch`/`#else_branch`: an empty body is `nil`, a
/// multi-statement body is a `begin` node -- never itself `const_type?`,
/// so it is skipped here just as it would fail that check upstream).
fn branch_statement(statements: Option<StatementsNode<'_>>) -> Option<Node<'_>> {
    let stmts = statements?.body();
    if stmts.len() == 1 {
        stmts.iter().next()
    } else {
        None
    }
}

/// RuboCop's `IfNode#branches`, recursing through an `elsif` chain.
fn collect_if_branches<'pr>(node: &IfNode<'pr>, out: &mut Vec<Node<'pr>>) {
    if let Some(b) = branch_statement(node.statements()) {
        out.push(b);
    }
    match node.subsequent() {
        Some(sub) if sub.kind() == NodeKind::IfNode => {
            if let Some(inner) = sub.as_if_node() {
                collect_if_branches(&inner, out);
            }
        }
        Some(sub) if sub.kind() == NodeKind::ElseNode => {
            if let Some(else_node) = sub.as_else_node() {
                if let Some(b) = branch_statement(else_node.statements()) {
                    out.push(b);
                }
            }
        }
        _ => {}
    }
}

/// `unless`'s branches (whitequark's `:if` type with swapped branches, no
/// `elsif` chain to recurse through).
fn collect_unless_branches<'pr>(node: &UnlessNode<'pr>, out: &mut Vec<Node<'pr>>) {
    if let Some(b) = branch_statement(node.statements()) {
        out.push(b);
    }
    if let Some(else_node) = node.else_clause() {
        if let Some(b) = branch_statement(else_node.statements()) {
            out.push(b);
        }
    }
}

/// RuboCop's `allowed_conditional_expression_on_rhs?`/`contains_constant?`.
fn allowed_conditional_expression_on_rhs(value: &Node<'_>) -> bool {
    let mut branches = Vec::new();
    match value.kind() {
        NodeKind::IfNode => {
            let Some(n) = value.as_if_node() else { return false };
            collect_if_branches(&n, &mut branches);
        }
        NodeKind::UnlessNode => {
            let Some(n) = value.as_unless_node() else { return false };
            collect_unless_branches(&n, &mut branches);
        }
        _ => return false,
    }
    branches
        .iter()
        .any(|b| matches!(b.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode))
}

/// RuboCop's `allowed_assignment?`.
fn allowed_assignment(value: Option<&Node<'_>>) -> bool {
    let Some(value) = value else { return false };
    is_block_const_or_casgn(value)
        || allowed_method_call_on_rhs(value)
        || is_class_or_struct_new(value)
        || allowed_conditional_expression_on_rhs(value)
}

/// Checks whether constant names are written using `SCREAMING_SNAKE_CASE`.
///
/// To avoid false positives, it ignores cases in which we cannot know for
/// certain the type of value that would be assigned to a constant.
///
/// # Examples
///
/// ```ruby
/// # bad
/// InchInCm = 2.54
/// INCHinCM = 2.54
/// Inch_In_Cm = 2.54
///
/// # good
/// INCH_IN_CM = 2.54
/// ```
#[derive(Debug, Clone)]
pub struct ConstantName;

impl ConstantName {
    /// RuboCop's `on_casgn`, for a write with a known RHS `value` (`None`
    /// for a bare masgn target, which has none).
    fn check(ctx: &mut Context<'_>, span: Span, name: &[u8], value: Option<&Node<'_>>) {
        if allowed_assignment(value) {
            return;
        }
        if is_screaming_snake_case(name) {
            return;
        }
        ctx.report(&Self::META, span, MSG);
    }
}

impl Rule for ConstantName {
    const META: RuleMeta = RuleMeta {
        name: "Naming/ConstantName",
        department: Department::Naming,
        summary: "Checks whether constant names are written using SCREAMING_SNAKE_CASE.",
        explanation: "\
Checks whether constant names are written using
SCREAMING_SNAKE_CASE.

To avoid false positives, it ignores cases in which we cannot know
for certain the type of value that would be assigned to a constant.

```ruby
# bad
InchInCm = 2.54
INCHinCM = 2.54
Inch_In_Cm = 2.54

# good
INCH_IN_CM = 2.54
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::ConstantOrWriteNode,
            NodeKind::ConstantPathOrWriteNode,
            NodeKind::ConstantTargetNode,
            NodeKind::ConstantPathTargetNode,
        ],
        config: &[],
        blind_spots: "\
`[[:digit:][:upper:]_]` is POSIX's character classes, matched here with Rust's Unicode-aware
`char::is_uppercase`/`is_ascii_digit`; this agrees with upstream on every accented Latin letter
its own doc comment calls out, but is not a byte-for-byte match of Oniguruma's POSIX `[:upper:]`
table for every script.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ConstantWriteNode => {
                if let Some(w) = node.as_constant_write_node() {
                    let value = w.value();
                    Self::check(ctx, w.name_loc().span(), w.name().as_slice(), Some(&value));
                }
            }
            NodeKind::ConstantOrWriteNode => {
                if let Some(w) = node.as_constant_or_write_node() {
                    let value = w.value();
                    Self::check(ctx, w.name_loc().span(), w.name().as_slice(), Some(&value));
                }
            }
            NodeKind::ConstantPathWriteNode => {
                if let Some(w) = node.as_constant_path_write_node() {
                    let target = w.target();
                    let Some(name) = target.name() else { return };
                    let value = w.value();
                    Self::check(ctx, target.name_loc().span(), name.as_slice(), Some(&value));
                }
            }
            NodeKind::ConstantPathOrWriteNode => {
                if let Some(w) = node.as_constant_path_or_write_node() {
                    let target = w.target();
                    let Some(name) = target.name() else { return };
                    let value = w.value();
                    Self::check(ctx, target.name_loc().span(), name.as_slice(), Some(&value));
                }
            }
            NodeKind::ConstantTargetNode => {
                if let Some(t) = node.as_constant_target_node() {
                    Self::check(ctx, t.location().span(), t.name().as_slice(), None);
                }
            }
            NodeKind::ConstantPathTargetNode => {
                if let Some(t) = node.as_constant_path_target_node() {
                    let Some(name) = t.name() else { return };
                    Self::check(ctx, t.name_loc().span(), name.as_slice(), None);
                }
            }
            _ => {}
        }
    }
}
