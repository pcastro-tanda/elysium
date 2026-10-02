//! `Lint/LiteralAssignmentInCondition`, ported from RuboCop's
//! `lib/rubocop/cop/lint/literal_assignment_in_condition.rb`.
//!
//! # Prism shape
//!
//! Upstream's `on_if`/`on_while`/`on_until` traverse their condition
//! looking for any whitequark `EQUALS_ASSIGNMENTS` node
//! (`lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`/`casgn`/`masgn`) whose right-hand
//! side is entirely literal. The traversal stops descending into a nested
//! block/method-definition's own body (an assignment there belongs to that
//! inner scope), which [`walk_condition`] reproduces with Prism's
//! `BlockNode`/`DefNode`.
//!
//! `collection[index] = 42` and `obj.attr = 42` are not in
//! `EQUALS_ASSIGNMENTS` upstream (they are plain `send`/`csend` nodes to
//! `[]=`/`attr=`), so they are deliberately left unmatched here too --
//! Prism represents both the same way, as an ordinary `CallNode`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Upstream's `MSG`.
const MSG: &str = "Don't use literal assignment `= {literal}` in conditional, should be `==` or \
                    non-literal operand.";

/// Checks for literal assignments in the conditions of `if`, `while`, and `until`.
#[derive(Debug, Clone)]
pub struct LiteralAssignmentInCondition;

impl Rule for LiteralAssignmentInCondition {
    const META: RuleMeta = RuleMeta {
        name: "Lint/LiteralAssignmentInCondition",
        department: Department::Lint,
        summary: "Checks for literal assignments in the conditions of `if`, `while`, and `until`.",
        explanation: "\
```ruby
# bad
if x = 42
  do_something
end

# good
if x == 42
  do_something
end

# good
if x = y
  do_something
end
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode, NodeKind::WhileNode, NodeKind::UntilNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let predicate = match node.kind() {
            NodeKind::IfNode => node.as_if_node().map(|n| n.predicate()),
            NodeKind::UnlessNode => node.as_unless_node().map(|n| n.predicate()),
            NodeKind::WhileNode => node.as_while_node().map(|n| n.predicate()),
            NodeKind::UntilNode => node.as_until_node().map(|n| n.predicate()),
            _ => None,
        };
        let Some(predicate) = predicate else { return };
        walk_condition(&predicate, ctx);
    }
}

/// One `=`-style assignment's operator location and right-hand side.
fn equals_asgn<'pr>(node: &Node<'pr>) -> Option<(Span, Node<'pr>)> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => {
            let n = node.as_local_variable_write_node()?;
            Some((n.operator_loc().span(), n.value()))
        }
        NodeKind::InstanceVariableWriteNode => {
            let n = node.as_instance_variable_write_node()?;
            Some((n.operator_loc().span(), n.value()))
        }
        NodeKind::ClassVariableWriteNode => {
            let n = node.as_class_variable_write_node()?;
            Some((n.operator_loc().span(), n.value()))
        }
        NodeKind::GlobalVariableWriteNode => {
            let n = node.as_global_variable_write_node()?;
            Some((n.operator_loc().span(), n.value()))
        }
        NodeKind::ConstantWriteNode => {
            let n = node.as_constant_write_node()?;
            Some((n.operator_loc().span(), n.value()))
        }
        NodeKind::ConstantPathWriteNode => {
            let n = node.as_constant_path_write_node()?;
            Some((n.operator_loc().span(), n.value()))
        }
        NodeKind::MultiWriteNode => {
            let n = node.as_multi_write_node()?;
            Some((n.operator_loc().span(), n.value()))
        }
        _ => None,
    }
}

/// The span of `node`'s own body, when `node` is a scope boundary
/// (`BlockNode`/`DefNode`) whose body should not be descended into --
/// upstream's `scope_body?`.
fn scope_body_span(node: &Node<'_>) -> Option<Span> {
    match node.kind() {
        NodeKind::BlockNode => node.as_block_node()?.body().map(|b| b.span()),
        NodeKind::DefNode => node.as_def_node()?.body().map(|b| b.span()),
        _ => None,
    }
}

/// Upstream's `traverse_node`: depth-first search for an `=`-style
/// assignment, skipping a nested block/def's own body.
fn walk_condition(node: &Node<'_>, ctx: &mut Context<'_>) {
    if let Some((op_span, rhs)) = equals_asgn(node) {
        if all_literals(&rhs) && !parallel_assignment_with_splat(&rhs) {
            report(ctx, op_span, &rhs);
        }
    }
    let skip = scope_body_span(node);
    for_each_child(node, |child| {
        if skip.is_some_and(|s| s == child.span()) {
            return;
        }
        walk_condition(child, ctx);
    });
}

/// Upstream's `all_literals?`.
fn all_literals(node: &Node<'_>) -> bool {
    match node.kind() {
        // `dstr`/`xstr` (whitequark) fall through to the wildcard below:
        // neither is ever literal, matching upstream's explicit `when
        // :dstr, :xstr then false` branch (dropped here as its own arm
        // since it returns the same `false` the wildcard already does).
        NodeKind::ArrayNode => {
            node.as_array_node().is_some_and(|a| a.elements().iter().all(|e| all_literals(&e)))
        }
        NodeKind::HashNode => {
            node.as_hash_node().is_some_and(|h| h.elements().iter().all(|e| assoc_literal(&e)))
        }
        NodeKind::KeywordHashNode => node
            .as_keyword_hash_node()
            .is_some_and(|h| h.elements().iter().all(|e| assoc_literal(&e))),
        NodeKind::StringNode
        | NodeKind::IntegerNode
        | NodeKind::FloatNode
        | NodeKind::SymbolNode
        | NodeKind::InterpolatedSymbolNode
        | NodeKind::RegularExpressionNode
        | NodeKind::InterpolatedRegularExpressionNode
        | NodeKind::TrueNode
        | NodeKind::FalseNode
        | NodeKind::NilNode
        | NodeKind::RangeNode
        | NodeKind::RationalNode
        | NodeKind::ImaginaryNode => true,
        _ => false,
    }
}

/// A hash pair is literal only when both its key and value are; a `**splat`
/// element (`AssocSplatNode`) is never literal.
fn assoc_literal(node: &Node<'_>) -> bool {
    node.as_assoc_node()
        .is_some_and(|assoc| all_literals(&assoc.key()) && all_literals(&assoc.value()))
}

/// Upstream's `parallel_assignment_with_splat_operator?`.
fn parallel_assignment_with_splat(node: &Node<'_>) -> bool {
    node.as_array_node()
        .is_some_and(|a| a.elements().first().is_some_and(|e| e.kind() == NodeKind::SplatNode))
}

#[allow(clippy::cast_possible_truncation)]
fn report(ctx: &mut Context<'_>, op_span: Span, rhs: &Node<'_>) {
    let span = Span::new(op_span.start, rhs.span().end);
    let literal = String::from_utf8_lossy(ctx.text(rhs.span())).into_owned();
    let message = MSG.replacen("{literal}", &literal, 1);
    ctx.report(&LiteralAssignmentInCondition::META, span, message);
}
