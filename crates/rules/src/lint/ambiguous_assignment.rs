//! `Lint/AmbiguousAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/lint/ambiguous_assignment.rb` plus the `CheckAssignment`
//! mixin it includes.
//!
//! Upstream's `CheckAssignment` dispatches on every whitequark assignment
//! type (`lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`/`casgn`/`masgn`/`op_asgn`/
//! `or_asgn`/`and_asgn`) plus `send`/`csend` for a setter call, but the
//! `MISTAKES` table only ever matches a plain `=` immediately followed by
//! `-`/`+`/`*`/`!` -- a shape only the equals-assignment family
//! (`EQUALS_ASSIGNMENTS` in rubocop-ast, plus a `CallNode` setter/indexer)
//! can produce; the shorthand family's operator is never a bare `=`. This is
//! confirmed by upstream's own spec, which only exercises
//! `x`/`@x`/`@@x`/`$x`/`X`/`obj.foo`/`arr[0]`/`hash[:key]`/`self.foo`/
//! `obj&.foo`/`h[k]` as left-hand sides. So only `LocalVariableWriteNode`,
//! `InstanceVariableWriteNode`, `ClassVariableWriteNode`,
//! `GlobalVariableWriteNode`, `ConstantWriteNode`, `ConstantPathWriteNode`,
//! `MultiWriteNode`, and `CallNode` (via `equal_loc`, covering both
//! attribute and index writers, and safe navigation since whitequark's
//! `csend` collapses into the same `CallNode` kind) are subscribed here.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks for mistyped shorthand assignments.
#[derive(Debug, Clone)]
pub struct AmbiguousAssignment;

impl Rule for AmbiguousAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Lint/AmbiguousAssignment",
        department: Department::Lint,
        summary: "Checks for mistyped shorthand assignments.",
        explanation: "\
Checks for mistyped shorthand assignments.

```ruby
# bad
x =- y
x =+ y
x =* y
x =! y

# good
x -= y # or x = -y
x += y # or x = +y
x *= y # or x = *y
x != y # or x = !y
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::LocalVariableWriteNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::ClassVariableWriteNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::MultiWriteNode,
            NodeKind::CallNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some((operator_span, rhs_span)) = operator_and_rhs(node) else { return };
        let Some(end) = operator_span.end.checked_sub(1) else { return };
        let range = Span::new(end, rhs_span.start + 1);
        let source = ctx.text(range);
        let Some(replacement) = mistake_replacement(source) else { return };

        let message = format!("Suspicious assignment detected. Did you mean `{replacement}`?");
        ctx.report(&Self::META, range, message);
    }
}

/// `MISTAKES` from upstream: a mistyped shorthand operator (parsed as a
/// plain `=` directly followed by a unary operator) and the shorthand the
/// author likely meant.
fn mistake_replacement(source: &[u8]) -> Option<&'static str> {
    match source {
        b"=-" => Some("-="),
        b"=+" => Some("+="),
        b"=*" => Some("*="),
        b"=!" => Some("!="),
        _ => None,
    }
}

/// `CheckAssignment#check_assignment`'s `node.loc.operator` and `rhs`: the
/// `=` location and the assigned expression's span, for every node kind
/// `CheckAssignment` dispatches on that can ever produce a bare `=`.
fn operator_and_rhs(node: &Node<'_>) -> Option<(Span, Span)> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => {
            let n = node.as_local_variable_write_node()?;
            Some((n.operator_loc().span(), n.value().span()))
        }
        NodeKind::InstanceVariableWriteNode => {
            let n = node.as_instance_variable_write_node()?;
            Some((n.operator_loc().span(), n.value().span()))
        }
        NodeKind::ClassVariableWriteNode => {
            let n = node.as_class_variable_write_node()?;
            Some((n.operator_loc().span(), n.value().span()))
        }
        NodeKind::GlobalVariableWriteNode => {
            let n = node.as_global_variable_write_node()?;
            Some((n.operator_loc().span(), n.value().span()))
        }
        NodeKind::ConstantWriteNode => {
            let n = node.as_constant_write_node()?;
            Some((n.operator_loc().span(), n.value().span()))
        }
        NodeKind::ConstantPathWriteNode => {
            let n = node.as_constant_path_write_node()?;
            Some((n.operator_loc().span(), n.value().span()))
        }
        NodeKind::MultiWriteNode => {
            let n = node.as_multi_write_node()?;
            Some((n.operator_loc().span(), n.value().span()))
        }
        NodeKind::CallNode => {
            let call = node.as_call_node()?;
            let equal_loc = call.equal_loc()?;
            let rhs = call.arguments()?.arguments().last()?;
            Some((equal_loc.span(), rhs.span()))
        }
        _ => None,
    }
}
