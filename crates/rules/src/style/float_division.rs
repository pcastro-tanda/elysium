//! `Style/FloatDivision`, ported from RuboCop's
//! `lib/rubocop/cop/style/float_division.rb`.
//!
//! Upstream's `offense_condition?` first calls `node.receiver.receiver` and
//! `node.first_argument.receiver`, relying on rubocop-ast's base `Node`
//! class defining a *generic* `receiver` node-matcher
//! (`{(send $_ ...) (any_block (call $_ ...) ...)}`) that safely returns
//! `nil` for any node that is not itself a send (rather than raising
//! `NoMethodError`, as a hand-written `attr_reader`-style accessor would).
//! [`generic_receiver`] mirrors that for the `CallNode` half of the pattern;
//! the `any_block`-wrapped-call half is not implemented (see the type's own
//! doc), which only matters for a division operand that is itself a
//! block-carrying call -- not exercised by any fixture.
//!
//! Prism represents a source-level `(...)` grouping as its own
//! `ParenthesesNode` (wrapping a `StatementsNode`), unlike whitequark, which
//! elides the wrapper for a single statement. This stands in for upstream's
//! `argument.begin_type?` check in the `fdiv` correction.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_LEFT: &str = "Prefer using `.to_f` on the left side.";
const MSG_RIGHT: &str = "Prefer using `.to_f` on the right side.";
const MSG_SINGLE: &str = "Prefer using `.to_f` on one side only.";
const MSG_FDIV: &str = "Prefer using `fdiv` for float divisions.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    LeftCoerce,
    RightCoerce,
    SingleCoerce,
    Fdiv,
}

/// The `CallNode` half of rubocop-ast's generic `Node#receiver` matcher: the
/// receiver of `node`, if `node` is itself a method call (which includes a
/// bare, receiverless call, whose own receiver is `None`/Ruby `nil`).
fn generic_receiver<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    node.as_call_node().and_then(|call| call.receiver())
}

/// RuboCop's `to_f_method?`: `(send !nil? :to_f)` -- a `.to_f` dispatch with
/// an explicit (non-implicit-self) receiver.
fn is_to_f_call(node: &Node<'_>) -> bool {
    node.as_call_node()
        .is_some_and(|call| call.receiver().is_some() && call.name().as_slice() == b"to_f")
}

/// RuboCop's `regexp_last_match?`: `Regexp.last_match(int)`/`::Regexp.last_match(int)`,
/// or a numbered back-reference (`$1`..`$9`, Prism's `NumberedReferenceReadNode`).
fn is_regexp_last_match(node: Option<Node<'_>>) -> bool {
    let Some(node) = node else { return false };
    if node.as_numbered_reference_read_node().is_some() {
        return true;
    }
    let Some(call) = node.as_call_node() else { return false };
    if call.name().as_slice() != b"last_match" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    if ext::const_name(&receiver).as_deref() != Some("Regexp") {
        return false;
    }
    let Some(arguments) = call.arguments() else { return false };
    let list = arguments.arguments();
    list.len() == 1 && list.first().is_some_and(|arg| arg.as_integer_node().is_some())
}

/// RuboCop's `add_to_f_method`'s own guard: `node.send_type? && node.method?(:to_f)`.
fn already_to_f(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| call.name().as_slice() == b"to_f")
}

/// RuboCop's `add_to_f_method`.
fn add_to_f(node: &Node<'_>, edits: &mut Vec<Edit>) {
    if !already_to_f(node) {
        edits.push(Edit::insert(node.span().end, b".to_f".to_vec()));
    }
}

/// RuboCop's `remove_to_f_method`: deletes the `.to_f` dot and selector.
fn remove_to_f(node: &Node<'_>, edits: &mut Vec<Edit>) {
    let Some(call) = node.as_call_node() else { return };
    let (Some(operator), Some(message)) = (call.call_operator_loc(), call.message_loc()) else {
        return;
    };
    edits.push(Edit::delete(Span::new(operator.span().start, message.span().end)));
}

/// RuboCop's `correct_from_slash_to_fdiv`.
fn fdiv_fix(left: &Node<'_>, right: &Node<'_>, left_to_f: bool, right_to_f: bool) -> Fix {
    let mut edits = Vec::new();
    if left_to_f {
        remove_to_f(left, &mut edits);
    }
    let argument_parenthesized = right.kind() == NodeKind::ParenthesesNode;
    let middle = Span::new(left.span().end, right.span().start);
    let replacement: &[u8] = if argument_parenthesized { b".fdiv" } else { b".fdiv(" };
    edits.push(Edit::replace(middle, replacement.to_vec()));
    if right_to_f {
        remove_to_f(right, &mut edits);
    }
    if !argument_parenthesized {
        edits.push(Edit::insert(right.span().end, b")".to_vec()));
    }
    Fix { applicability: Applicability::Unsafe, edits }
}

/// For performing float division, coerce one side only.
#[derive(Debug, Clone)]
pub struct FloatDivision {
    style: Style,
}

impl Rule for FloatDivision {
    const META: RuleMeta = RuleMeta {
        name: "Style/FloatDivision",
        department: Department::Style,
        summary: "For performing float division, coerce one side only.",
        explanation: "\
Checks for division with integers coerced to floats.
It is recommended to either always use `fdiv` or coerce one side only.
This cop also provides other options for code consistency.

For `Regexp.last_match` and nth reference (e.g., `$1`), it assumes that the value
is a string matched by a regular expression, and allows conversion with `#to_f`.

@safety
  This cop is unsafe, because if the operand variable is a string object
  then `#to_f` will be removed and an error will occur.

  ```ruby
  a = '1.2'
  b = '3.4'
  a.to_f / b.to_f # Both `to_f` calls are required here
  ```

With `EnforcedStyle: single_coerce` (default):

```ruby
# bad
a.to_f / b.to_f

# good
a.to_f / b
a / b.to_f
```

With `EnforcedStyle: left_coerce`:

```ruby
# bad
a / b.to_f
a.to_f / b.to_f

# good
a.to_f / b
```

With `EnforcedStyle: right_coerce`:

```ruby
# bad
a.to_f / b
a.to_f / b.to_f

# good
a / b.to_f
```

With `EnforcedStyle: fdiv`:

```ruby
# bad
a / b.to_f
a.to_f / b
a.to_f / b.to_f

# good
a.fdiv(b)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("single_coerce"),
            allowed: &["left_coerce", "right_coerce", "single_coerce", "fdiv"],
            doc: "Which side(s) of a float division to coerce.",
        }],
        blind_spots: "\
Upstream's `offense_condition?` reaches `node.receiver.receiver`/
`node.first_argument.receiver` through rubocop-ast's generic `Node#receiver`
node-matcher, which also matches a block-wrapping-a-call receiver
(`any_block (call $_ ...)`); this port's `generic_receiver` only implements
the plain-call half, so a division operand that is itself a block-carrying
call (e.g. `foo.bar { }.to_f / baz`) is not walked through to its own
receiver for the `Regexp.last_match`/nth-ref exemption. No fixture or spec
example exercises that shape.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "left_coerce" => Style::LeftCoerce,
            "right_coerce" => Style::RightCoerce,
            "fdiv" => Style::Fdiv,
            _ => Style::SingleCoerce,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() || call.name().as_slice() != b"/" {
            return;
        }
        let Some(left) = call.receiver() else { return };
        let Some(arguments) = call.arguments() else { return };
        let Some(right) = arguments.arguments().first() else { return };

        if is_regexp_last_match(generic_receiver(&left))
            || is_regexp_last_match(generic_receiver(&right))
        {
            return;
        }

        let left_to_f = is_to_f_call(&left);
        let right_to_f = is_to_f_call(&right);
        let offense = match self.style {
            Style::LeftCoerce => right_to_f,
            Style::RightCoerce => left_to_f,
            Style::SingleCoerce => left_to_f && right_to_f,
            Style::Fdiv => left_to_f || right_to_f,
        };
        if !offense {
            return;
        }

        let message = match self.style {
            Style::LeftCoerce => MSG_LEFT,
            Style::RightCoerce => MSG_RIGHT,
            Style::SingleCoerce => MSG_SINGLE,
            Style::Fdiv => MSG_FDIV,
        };
        let span = call.as_node().span();
        let fix = match self.style {
            Style::LeftCoerce | Style::SingleCoerce => {
                let mut edits = Vec::new();
                add_to_f(&left, &mut edits);
                remove_to_f(&right, &mut edits);
                Fix { applicability: Applicability::Unsafe, edits }
            }
            Style::RightCoerce => {
                let mut edits = Vec::new();
                remove_to_f(&left, &mut edits);
                add_to_f(&right, &mut edits);
                Fix { applicability: Applicability::Unsafe, edits }
            }
            Style::Fdiv => fdiv_fix(&left, &right, left_to_f, right_to_f),
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }
}
