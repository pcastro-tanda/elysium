//! `Lint/UselessOr`, ported from RuboCop's `lib/rubocop/cop/lint/useless_or.rb`.
//!
//! # Climbing to the enclosing `or` without typed ancestor access
//!
//! Upstream's second `on_or` branch (`truthy_return_value_method?(node.rhs)`)
//! reports on `node.parent` (climbing past one `begin`-wrapping level, i.e.
//! parentheses) rather than on `node` itself -- e.g. for `foo || x.__id__ ||
//! fallback`, which Ruby's left-associative `||` parses as `(foo ||
//! x.__id__) || fallback`, the *inner* `OrNode` (`foo || x.__id__`) is the
//! one whose `rhs` (`x.__id__`) matches, but the offense/fix target is the
//! *outer* node. [`Context::ancestors`] only exposes `NodeInfo` (kind/span,
//! no typed field access), so this rule instead tracks a private stack of
//! [`Frame`]s -- one per currently-open [`NodeKind::OrNode`] (its own
//! operator/lhs/rhs spans) or [`NodeKind::ParenthesesNode`] (a bare marker)
//! -- pushed on `enter` *after* running both branches' checks (so the stack
//! reflects strict ancestors, never the current node) and popped on
//! `leave`. [`enclosing_or`] then reproduces `parent = parent.parent if
//! parent&.begin_type?; parent&.or_type?` by looking at the top one or two
//! frames.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `TRUTHY_RETURN_VALUE_METHODS`.
const TRUTHY_METHODS: &[&[u8]] = &[
    b"to_a",
    b"to_c",
    b"to_d",
    b"to_i",
    b"to_f",
    b"to_h",
    b"to_r",
    b"to_s",
    b"to_sym",
    b"intern",
    b"inspect",
    b"hash",
    b"object_id",
    b"__id__",
];

/// One currently-open ancestor relevant to [`enclosing_or`]: either an
/// `OrNode`'s own operator/lhs/rhs/whole spans, or a bare parentheses
/// marker.
#[derive(Debug, Clone, Copy)]
enum Frame {
    Or { operator_span: Span, lhs_span: Span, rhs_span: Span, own_span: Span },
    Paren,
}

/// Checks for useless OR (`||` and `or`) expressions.
#[derive(Debug, Clone)]
pub struct UselessOr {
    /// See the module doc comment.
    stack: Vec<Frame>,
}

impl Rule for UselessOr {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UselessOr",
        department: Department::Lint,
        summary: "Checks for useless OR expressions.",
        explanation: "\
Checks for useless OR (`||` and `or`) expressions.

Some methods always return a truthy value, even when called on `nil`
(e.g. `nil.to_i` evaluates to `0`). Therefore, OR expressions appended
after these methods will never evaluate.

@safety
  As shown in the examples below, there are generally two possible ways to
  correct the offense, but this cop's autocorrection always chooses the
  option that preserves the current behavior. While this does not change
  how the code behaves, that option is not necessarily the appropriate fix
  in every situation. For this reason, the autocorrection provided by this
  cop is considered unsafe.

```ruby
# bad
x.to_a || fallback
x.to_s || fallback
x.to_s or fallback

# good - if fallback is same as return value of method called on nil
x.to_a # nil.to_a returns []
x.to_s # nil.to_s returns ''

# good - if the intention is not to call the method on nil
x&.to_a || fallback
x&.to_s || fallback
x&.to_s or fallback
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::OrNode, NodeKind::ParenthesesNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { stack: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::ParenthesesNode { .. } => self.stack.push(Frame::Paren),
            Node::OrNode { .. } => {
                let or = node.as_or_node().expect("kind matched");
                let left = or.left();
                let right = or.right();
                let frame = Frame::Or {
                    operator_span: or.operator_loc().span(),
                    lhs_span: left.span(),
                    rhs_span: right.span(),
                    own_span: node.span(),
                };
                if is_truthy_return_value_call(&left) {
                    report_offense(ctx, frame, &left);
                } else if is_truthy_return_value_call(&right) {
                    if let Some(outer) = enclosing_or(&self.stack) {
                        report_offense(ctx, outer, &right);
                    }
                }
                self.stack.push(frame);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if matches!(node, Node::OrNode { .. } | Node::ParenthesesNode { .. }) {
            self.stack.pop();
        }
    }
}

/// RuboCop's `truthy_return_value_method?`: `(send _ %TRUTHY_RETURN_VALUE_METHODS)`.
/// A `send`-only node pattern never matches a `csend` node, so safe
/// navigation is excluded -- there is no `on_csend` alias here.
fn is_truthy_return_value_call(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    !call.is_safe_navigation() && TRUTHY_METHODS.contains(&call.name().as_slice())
}

/// RuboCop's `parent = node.parent; parent = parent.parent if
/// parent&.begin_type?; parent&.or_type?`.
fn enclosing_or(stack: &[Frame]) -> Option<Frame> {
    match stack.last()? {
        Frame::Or { .. } => stack.last().copied(),
        Frame::Paren => {
            let grandparent = stack.get(stack.len().checked_sub(2)?)?;
            matches!(grandparent, Frame::Or { .. }).then_some(*grandparent)
        }
    }
}

/// RuboCop's `report_offense`.
fn report_offense(ctx: &mut Context<'_>, or_node: Frame, truthy_node: &Node<'_>) {
    let Frame::Or { operator_span, lhs_span, rhs_span, own_span } = or_node else {
        return;
    };
    let offense_span = Span::new(operator_span.start, rhs_span.end);
    let lhs_text = String::from_utf8_lossy(ctx.text(truthy_node.span())).into_owned();
    let rhs_text = String::from_utf8_lossy(ctx.text(rhs_span)).into_owned();
    let message = format!(
        "`{rhs_text}` will never evaluate because `{lhs_text}` always returns a truthy value."
    );
    let replacement = ctx.text(lhs_span).to_vec();
    ctx.report_with_fix(
        &UselessOr::META,
        offense_span,
        message,
        Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(own_span, replacement.into_boxed_slice())],
        },
    );
}
