//! `Style/MinMax`, ported from RuboCop's
//! `lib/rubocop/cop/style/min_max.rb`.
//!
//! Upstream matches `({array return} (send [$_receiver !nil?] :min) (send
//! [$_receiver !nil?] :max))`: an array literal (explicit `[a, b]` or the
//! implicit array built by `bar = a, b`) or a `return` with exactly two
//! elements, the first a receiver-having, argument-less `.min` call and the
//! second a receiver-having, argument-less `.max` call on the *same*
//! receiver (compared here by source text, matching the rest of this
//! codebase's node-equality convention, e.g.
//! `binary_operator_with_identical_operands.rs`).
//!
//! For the array case the offense/replacement span is the whole node's own
//! span (which, for an implicit array, already excludes the surrounding
//! `bar = `/brackets, matching upstream's `node.source_range`). For `return`
//! it is the joined span of the first and last argument, matching upstream's
//! `argument_range` (`first_argument_range.join(last_argument_range)`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use `Enumerable#minmax` instead of `Enumerable#min` and `Enumerable#max` in conjunction.
#[derive(Debug, Clone)]
pub struct MinMax;

impl Rule for MinMax {
    const META: RuleMeta = RuleMeta {
        name: "Style/MinMax",
        department: Department::Style,
        summary: "Use `Enumerable#minmax` instead of `Enumerable#min` and `Enumerable#max` in conjunction.",
        explanation: "\
Checks for potential uses of `Enumerable#minmax`.

```ruby
# bad
bar = [foo.min, foo.max]
return foo.min, foo.max

# good
bar = foo.minmax
return foo.minmax
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ArrayNode, NodeKind::ReturnNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (elements, offender_span) = match node.kind() {
            NodeKind::ArrayNode => {
                let array = node.as_array_node().expect("kind matched");
                let elements: Vec<Node<'_>> = array.elements().iter().collect();
                (elements, node.span())
            }
            NodeKind::ReturnNode => {
                let ret = node.as_return_node().expect("kind matched");
                let Some(args) = ret.arguments() else { return };
                let elements: Vec<Node<'_>> = args.arguments().iter().collect();
                let Some(first) = elements.first() else { return };
                let Some(last) = elements.last() else { return };
                let offender_span = Span::new(first.span().start, last.span().end);
                (elements, offender_span)
            }
            _ => return,
        };
        let [first, second] = elements.as_slice() else { return };
        let Some(receiver_span) = min_max_candidate(ctx, first, second) else { return };

        let receiver_text = String::from_utf8_lossy(ctx.text(receiver_span)).into_owned();
        let offender_text = String::from_utf8_lossy(ctx.text(offender_span)).into_owned();
        let message = format!("Use `{receiver_text}.minmax` instead of `{offender_text}`.");
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(
                offender_span,
                format!("{receiver_text}.minmax").into_bytes(),
            )],
        };
        ctx.report_with_fix(&Self::META, offender_span, message, fix);
    }
}

/// RuboCop's `min_max_candidate` node matcher: `first` must be an
/// argument-less `.min` call and `second` an argument-less `.max` call, both
/// on the same non-nil receiver. Returns that receiver's span.
fn min_max_candidate(ctx: &Context<'_>, first: &Node<'_>, second: &Node<'_>) -> Option<Span> {
    let min_receiver = min_max_call(first, b"min")?;
    let max_receiver = min_max_call(second, b"max")?;
    if ctx.text(min_receiver.span()) != ctx.text(max_receiver.span()) {
        return None;
    }
    Some(min_receiver.span())
}

/// A receiver-having, argument-less call named `name`; returns the receiver.
fn min_max_call<'pr>(node: &Node<'pr>, name: &[u8]) -> Option<Node<'pr>> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != name {
        return None;
    }
    if let Some(args) = call.arguments() {
        if !args.arguments().is_empty() {
            return None;
        }
    }
    call.receiver()
}
