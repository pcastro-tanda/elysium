//! `Style/RedundantSort`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_sort.rb`.
//!
//! Upstream matches the *sort* call (`on_send` restricted to `sort`/
//! `sort_by`) and then inspects its parent/grandparent for an accessor
//! send. In Prism, a literal block attached to `sort`/`sort_by` is a field
//! on the `CallNode` itself (`CallNode::block()`), not a wrapping `block`
//! node as in whitequark, so the accessor is always the immediate parent
//! of the sort call -- this port instead subscribes to `CallNode` and, for
//! every `first`/`last`/`[]`/`at`/`slice` call, inspects its *receiver*.
//!
//! `CallNode::block()` also yields a `BlockArgumentNode` for `&:foo`
//! (whitequark's parser folds that into the regular argument list as a
//! `block-pass` node instead), so a bare literal-block check
//! (`as_block_node().is_some()`) distinguishes it from an actual `{ }`/
//! `do...end` block and it is counted as a positional argument instead --
//! reproducing the three node-pattern alternatives upstream's
//! `redundant_sort?` matches for the sort call: `sort` with zero arguments
//! (block or not), `sort_by` with exactly one non-block argument (`&:foo`
//! or otherwise), and `sort_by` with a literal block and zero arguments.
//!
//! `-1` folds into `IntegerNode` itself in Prism, so no unary-minus-call
//! case is needed for `[-1]`/`at(-1)`/`slice(-1)`.
//!
//! Upstream's `operator_keyword?` on the accessor's parent is true for
//! *both* `&&`/`||` and `and`/`or` (whitequark folds them into the same
//! `and`/`or` node types, differing only in `loc.operator`'s text) --
//! Prism's `AndNode`/`OrNode` are the same across both spellings, so a
//! plain kind check reproduces it. Because the engine only exposes
//! ancestor `(kind, span)` pairs, not typed nodes, this rule keeps its own
//! stack of enclosing `AndNode`/`OrNode` operator locations, pushed on
//! `enter` and popped on `leave`, to recover `parent.loc.operator` for the
//! `replace_with_logical_operator` rewrite.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use `min` instead of `sort.first`, `max_by` instead of `sort_by...last`, etc.
#[derive(Debug, Clone, Default)]
pub struct RedundantSort {
    /// Operator-token spans of enclosing `AndNode`/`OrNode` ancestors,
    /// innermost last. See the module doc for why the engine's ancestor
    /// list alone cannot recover this.
    logical_ops: Vec<Span>,
}

impl Rule for RedundantSort {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantSort",
        department: Department::Style,
        summary: "Use `min` instead of `sort.first`, `max_by` instead of `sort_by...last`, etc.",
        explanation: "\
Identifies instances of sorting and then taking only the first or last \
element. The same behavior can be accomplished without a relatively \
expensive sort by using `Enumerable#min` instead of sorting and taking the \
first element and `Enumerable#max` instead of sorting and taking the last \
element. Similarly, `Enumerable#min_by` and `Enumerable#max_by` can replace \
`Enumerable#sort_by` calls after which only the first or last element is \
used.\n\n\
This cop is unsafe, because `sort...last` and `max` may not return the \
same element in all cases: where there are multiple elements for which \
`a <=> b == 0`, or where the transformation done by the `sort_by` block \
has the same result, `sort.last` returns the last such element but `max` \
returns the first.\n\n\
```ruby\n\
# bad\n\
[2, 1, 3].sort.first\n\
[2, 1, 3].sort[0]\n\
[2, 1, 3].sort.at(0)\n\
[2, 1, 3].sort.slice(0)\n\n\
# good\n\
[2, 1, 3].min\n\n\
# bad\n\
[2, 1, 3].sort.last\n\
[2, 1, 3].sort[-1]\n\n\
# good\n\
[2, 1, 3].max\n\n\
# bad\n\
arr.sort_by(&:foo).first\n\n\
# good\n\
arr.min_by(&:foo)\n\
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::AndNode, NodeKind::OrNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::AndNode => {
                let and = node.as_and_node().expect("kind matched");
                self.logical_ops.push(and.operator_loc().span());
            }
            NodeKind::OrNode => {
                let or = node.as_or_node().expect("kind matched");
                self.logical_ops.push(or.operator_loc().span());
            }
            NodeKind::CallNode => self.check_call(node, ctx),
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if matches!(node.kind(), NodeKind::AndNode | NodeKind::OrNode) {
            self.logical_ops.pop();
        }
    }
}

impl RedundantSort {
    fn check_call(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        let name = call.name();
        let name = name.as_slice();
        if !matches!(name, b"first" | b"last" | b"[]" | b"at" | b"slice") {
            return;
        }
        let Some(is_min) = accessor_is_min(name, &call) else { return };
        let Some(receiver) = call.receiver() else { return };
        let Some(sort_call) = receiver.as_call_node() else { return };
        let Some(suffix) = sort_suffix(&sort_call) else { return };
        let Some(sort_message) = sort_call.message_loc() else { return };
        let sorter = sort_call.name();
        let sorter = sorter.as_slice();

        let accessor_message_start = call
            .message_loc()
            .map(|loc| loc.span().start)
            .or_else(|| call.opening_loc().map(|loc| loc.span().start))
            .expect("a call has a message or an opening delimiter");
        let accessor_end = node.span().end;
        let accessor_source = ctx.text(Span::new(accessor_message_start, accessor_end)).to_vec();

        let suggestion = format!("{}{}", if is_min { "min" } else { "max" }, suffix);
        let message = format!(
            "Use `{suggestion}` instead of `{}...{}`.",
            String::from_utf8_lossy(sorter),
            String::from_utf8_lossy(&accessor_source)
        );

        let offense_span = Span::new(sort_message.span().start, accessor_end);

        let accessor_start =
            call.call_operator_loc().map_or(accessor_message_start, |loc| loc.span().start);

        let mut edits = vec![
            Edit::delete(Span::new(accessor_start, accessor_end)),
            Edit::replace(sort_message.span(), suggestion.clone().into_bytes()),
        ];

        let parent_is_logical =
            ctx.parent().is_some_and(|p| matches!(p.kind, NodeKind::AndNode | NodeKind::OrNode));
        if parent_is_logical {
            if let Some(&operator_span) = self.logical_ops.last() {
                let operator_text = ctx.text(operator_span).to_vec();
                let mut insertion = vec![b' '];
                insertion.extend_from_slice(&operator_text);
                edits.push(Edit::insert(receiver.span().end, insertion));
                edits.push(Edit::delete(operator_span));
            }
        }

        ctx.report_with_fix(
            &Self::META,
            offense_span,
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// The receiver's `sort`/`sort_by` accepts no explicit arguments beyond an
/// optional literal block (`sort`), or exactly one argument that is not a
/// literal block -- typically `&:foo` -- xor a literal block with zero
/// other arguments (`sort_by`). Returns the method-name suffix used to
/// build the replacement (`""` for `sort`, `"_by"` for `sort_by`).
fn sort_suffix(sort_call: &CallNode<'_>) -> Option<&'static str> {
    let literal_block = sort_call.block().is_some_and(|b| b.as_block_node().is_some());
    let block_pass = sort_call.block().is_some() && !literal_block;
    let positional = sort_call.arguments().map_or(0, |a| a.arguments().len());
    let total = positional + usize::from(block_pass);
    match sort_call.name().as_slice() {
        b"sort" if total == 0 => Some(""),
        b"sort_by" if (literal_block && total == 0) || (!literal_block && total == 1) => {
            Some("_by")
        }
        _ => None,
    }
}

/// Whether the accessor call selects the first (`min`) or last (`max`)
/// element once sorted: `first`/`last` take no arguments, `[]`/`at`/
/// `slice` take exactly one integer literal argument, `0` or `-1`.
fn accessor_is_min(name: &[u8], call: &CallNode<'_>) -> Option<bool> {
    if call.block().is_some() {
        return None;
    }
    match name {
        b"first" if call.arguments().is_none() => Some(true),
        b"last" if call.arguments().is_none() => Some(false),
        b"[]" | b"at" | b"slice" => match single_int_arg(call) {
            Some(0) => Some(true),
            Some(-1) => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// The sole argument's value, when the call has exactly one integer
/// literal argument.
fn single_int_arg(call: &CallNode<'_>) -> Option<i32> {
    let args = call.arguments()?;
    let args = args.arguments();
    if args.len() != 1 {
        return None;
    }
    args.first()?.as_integer_node()?.value().try_into().ok()
}
