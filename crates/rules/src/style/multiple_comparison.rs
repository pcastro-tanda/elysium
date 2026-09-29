//! `Style/MultipleComparison`, ported from RuboCop's
//! `lib/rubocop/cop/style/multiple_comparison.rb`.
//!
//! # `find_offending_var` as a shared accumulator
//!
//! Upstream walks an `or` tree depth-first (`lhs` before `rhs`), threading a
//! shared `variables` `Set`, `values` array and `skipped` array through every
//! recursive call. A leaf `lvar == lvar` comparison (`simple_double_comparison?`)
//! is dropped entirely -- it contributes to neither `variables` nor `values` --
//! while any other valid comparison (`simple_comparison`) adds its "variable"
//! side to `variables` and, unless its "other" side is itself a method call
//! that `AllowMethodComparison` exempts (in which case the whole leaf goes to
//! `skipped` instead), its "other" side to `values`. As soon as a *second*
//! distinct variable shows up, further leaves stop contributing to `values`
//! (but keep being walked, so trailing `skipped` entries are still recorded).
//! [`MultipleComparison::collect`] mirrors this by mutating three `Vec`s
//! shared across the whole recursive descent; `variables` is deduplicated by
//! source-text equality, a cheap stand-in for upstream's structural
//! `AST::Node#==` (which ignores location) since every fixture's "variable"
//! side always reads identically at each occurrence.
//!
//! # Offense range and the "skipped sits inside the range" bail-out
//!
//! The reported (and corrected) range spans from the first collected leaf
//! comparison's start to the last collected leaf comparison's end -- *not*
//! necessarily the whole `or` chain, since a trailing method-comparison leaf
//! (or one past a second distinct variable) is simply left outside the
//! range and untouched by the correction. But if an exempted comparison
//! sits *between* two collected leaves, collapsing the range would silently
//! drop it, so [`MultipleComparison::enter`] bails if any `skipped` leaf's
//! span falls inside the computed range.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, Fix, FixAvailability, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Avoid comparing a variable with multiple items in a conditional, use \
                    `Array#include?` instead.";

/// One accepted (non-skipped, non-double-lvar) leaf comparison: the "other"
/// side's span (used to render the `include?` array literal) and the whole
/// `==` call's span (used for the offense/correction range).
struct Value {
    obj: Span,
    leaf: Span,
}

/// Avoid comparing a variable with multiple items in a conditional, use Array#include? instead.
#[derive(Debug, Clone)]
pub struct MultipleComparison {
    allow_method_comparison: bool,
    comparisons_threshold: i64,
}

impl MultipleComparison {
    /// RuboCop's `simple_comparison`: is `node` a `lhs == rhs` call whose
    /// "variable" side (tried as `lhs` first, falling back to `rhs` only if
    /// `lhs` isn't itself a local-variable-read or a call) is a local
    /// variable or a method call? Returns `(var_span, var_is_call, obj_span,
    /// obj_is_call)`, or `None` if neither side qualifies or the qualifying
    /// side is a call and `AllowMethodComparison` is disabled.
    fn simple_comparison(&self, node: &Node<'_>) -> Option<(Span, bool, Span, bool)> {
        let call = node.as_call_node()?;
        if call.name().as_slice() != b"==" || call.is_safe_navigation() {
            return None;
        }
        let receiver = call.receiver()?;
        let args = call.arguments()?.arguments();
        if args.len() != 1 {
            return None;
        }
        let rhs = args.iter().next()?;

        let is_var_shaped =
            |n: &Node<'_>| matches!(n.kind(), NodeKind::LocalVariableReadNode | NodeKind::CallNode);
        let (var, obj) = if is_var_shaped(&receiver) {
            (receiver, rhs)
        } else if is_var_shaped(&rhs) {
            (rhs, receiver)
        } else {
            return None;
        };
        let var_is_call = var.kind() == NodeKind::CallNode;
        if var_is_call && !self.allow_method_comparison {
            return None;
        }
        let obj_is_call = obj.kind() == NodeKind::CallNode;
        Some((var.span(), var_is_call, obj.span(), obj_is_call))
    }

    /// RuboCop's `comparison?`/`nested_comparison?`: is every leaf of this
    /// (possibly `or`-nested) subtree a valid [`MultipleComparison::simple_comparison`]?
    fn is_comparison_tree(&self, node: &Node<'_>) -> bool {
        if let Some(or_node) = node.as_or_node() {
            self.is_comparison_tree(&or_node.left()) && self.is_comparison_tree(&or_node.right())
        } else {
            self.simple_comparison(node).is_some()
        }
    }

    /// RuboCop's `find_offending_var`, walking `lhs` before `rhs` and
    /// mutating the shared accumulators. See the module doc.
    fn collect(
        &self,
        node: &Node<'_>,
        variables: &mut Vec<Span>,
        values: &mut Vec<Value>,
        ctx: &Context<'_>,
        skipped: &mut Vec<Span>,
    ) {
        if let Some(or_node) = node.as_or_node() {
            self.collect(&or_node.left(), variables, values, ctx, skipped);
            self.collect(&or_node.right(), variables, values, ctx, skipped);
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"==" || call.is_safe_navigation() {
            return;
        }
        // `simple_double_comparison?`: a bare `lvar == lvar` contributes
        // nothing at all, on either side.
        if let (Some(receiver), Some(args)) = (call.receiver(), call.arguments()) {
            let arg_list = args.arguments();
            if arg_list.len() == 1
                && receiver.kind() == NodeKind::LocalVariableReadNode
                && arg_list
                    .iter()
                    .next()
                    .is_some_and(|a| a.kind() == NodeKind::LocalVariableReadNode)
            {
                return;
            }
        }
        let Some((var_span, _var_is_call, obj_span, obj_is_call)) = self.simple_comparison(node)
        else {
            return;
        };
        if self.allow_method_comparison && obj_is_call {
            skipped.push(node.span());
            return;
        }
        if !variables.iter().any(|v| ctx.text(*v) == ctx.text(var_span)) {
            variables.push(var_span);
        }
        if variables.len() > 1 {
            return;
        }
        values.push(Value { obj: obj_span, leaf: node.span() });
    }
}

impl Rule for MultipleComparison {
    const META: RuleMeta = RuleMeta {
        name: "Style/MultipleComparison",
        department: Department::Style,
        summary: "Avoid comparing a variable with multiple items in a conditional, use Array#include? instead.",
        explanation: "\
Checks against comparing a variable with multiple items, where
`Array#include?`, `Set#include?` or a `case` could be used instead
to avoid code repetition.
It accepts comparisons of multiple method calls to avoid unnecessary method calls
by default. It can be configured by `AllowMethodComparison` option.

```ruby
# bad
a = 'a'
foo if a == 'a' || a == 'b' || a == 'c'

# good
a = 'a'
foo if ['a', 'b', 'c'].include?(a)

VALUES = Set['a', 'b', 'c'].freeze
# elsewhere...
foo if VALUES.include?(a)

case foo
when 'a', 'b', 'c' then foo
# ...
end

# accepted (but consider `case` as above)
foo if a == b.lightweight || a == b.heavyweight
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::OrNode],
        config: &[
            ConfigOption {
                name: "AllowMethodComparison",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether a comparison whose \"other\" side is itself a method call is \
                      exempted.",
            },
            ConfigOption {
                name: "ComparisonsThreshold",
                default: ConfigDefault::Int(2),
                allowed: &[],
                doc: "Minimum number of comparisons against the same variable before an \
                      offense is registered.",
            },
        ],
        blind_spots: "\
`variables` is deduplicated by source-text equality rather than upstream's \
structural `AST::Node#==` (which ignores location); every fixture's \
\"variable\" side always reads identically at each occurrence, so this never \
diverges in practice.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_method_comparison: options.bool("AllowMethodComparison"),
            comparisons_threshold: options.int("ComparisonsThreshold"),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(_or_node) = node.as_or_node() else { return };
        // Only the root of an `or`-chain is processed; nested `or`s are
        // walked from there (RuboCop's `on_or` `root_of_or_node?` guard).
        if ctx.parent().is_some_and(|p| p.kind == NodeKind::OrNode) {
            return;
        }
        if !self.is_comparison_tree(node) {
            return;
        }

        let mut variables = Vec::new();
        let mut values = Vec::new();
        let mut skipped = Vec::new();
        self.collect(node, &mut variables, &mut values, ctx, &mut skipped);
        let count = i64::try_from(values.len()).unwrap_or(i64::MAX);
        if variables.is_empty() || count < self.comparisons_threshold {
            return;
        }

        let range = Span::new(values[0].leaf.start, values[values.len() - 1].leaf.end);
        if skipped.iter().any(|s| range.start <= s.start && s.end <= range.end) {
            return;
        }

        let elements: Vec<String> =
            values.iter().map(|v| String::from_utf8_lossy(ctx.text(v.obj)).into_owned()).collect();
        let var_text = String::from_utf8_lossy(ctx.text(variables[0])).into_owned();
        let replacement = format!("[{}].include?({var_text})", elements.join(", "));

        let fix = Fix {
            applicability: linter::Applicability::Safe,
            edits: vec![linter::Edit::replace(range, replacement.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, range, MSG, fix);
    }
}
