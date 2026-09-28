//! `Lint/FloatComparison`, ported from RuboCop's
//! `lib/rubocop/cop/lint/float_comparison.rb`.
//!
//! # `float?`
//!
//! Upstream's `float?` recurses through whitequark's `:begin` node type,
//! which whitequark emits both for a keyword `begin...end` block and for any
//! parenthesized expression. Prism only reuses `BeginNode` for the keyword
//! form; a parenthesized expression is a distinct `ParenthesesNode` wrapping
//! a `StatementsNode`. [`is_float`] follows the latter (unwrapping to the
//! first statement, mirroring `node.children.first`) since that is the only
//! shape RuboCop's own node builder can ever produce as an operand here --
//! `x == (foo; 0.1)` is not valid Ruby syntax for a binary operand, so a
//! keyword `begin` never actually reaches this position.
//!
//! # `node.receiver&.float_type?`
//!
//! `float_send?`'s instance-method branch checks that the receiver is
//! *literally* a float literal node, not that it merely *produces* a float
//! (unlike the recursive `float?` used for arithmetic operands). This means
//! `x.to_f.abs == 0.1` is not recognized as a float via this branch --
//! matching upstream exactly, not a deviation introduced by this port.
//!
//! # `numeric_returning_method?`
//!
//! Ported verbatim from the upstream `case`: `angle`/`arg`/`phase` accept
//! only when the (literal float) receiver's source parses as a negative
//! `f64`; `ceil`/`floor`/`round`/`truncate` accept only when given a single
//! integer-literal argument whose source parses as a positive `i64`. Both
//! parses use the node's own source text, exactly as upstream's
//! `Float(node.receiver.source)`/`Integer(precision.source)`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG_EQUALITY`.
const MSG_EQUALITY: &str = "Avoid equality comparisons of floats as they are unreliable.";
/// RuboCop's `MSG_INEQUALITY`.
const MSG_INEQUALITY: &str = "Avoid inequality comparisons of floats as they are unreliable.";
/// RuboCop's `MSG_CASE`.
const MSG_CASE: &str = "Avoid float literal comparisons in case statements as they are unreliable.";

/// RuboCop's `FLOAT_RETURNING_METHODS`.
fn is_float_returning_method(name: &[u8]) -> bool {
    matches!(name, b"to_f" | b"Float" | b"fdiv")
}

/// RuboCop's `FLOAT_INSTANCE_METHODS`. `@-` is upstream's own (unusual)
/// spelling, kept verbatim.
fn is_float_instance_method(name: &[u8]) -> bool {
    matches!(
        name,
        b"@-" | b"abs" | b"magnitude" | b"modulo" | b"next_float" | b"prev_float" | b"quo"
    )
}

/// RuboCop-AST's `ARITHMETIC_OPERATORS`, checked by `arithmetic_operation?`.
fn is_arithmetic_operator(name: &[u8]) -> bool {
    matches!(name, b"+" | b"-" | b"*" | b"/" | b"%" | b"**")
}

/// Checks for the presence of precise comparison of floating point numbers.
#[derive(Debug, Clone, Default)]
pub struct FloatComparison;

impl FloatComparison {
    /// RuboCop's `literal_safe?`: a zero numeric literal, or a literal
    /// `nil`.
    fn is_literal_safe(node: &Node<'_>) -> bool {
        node.kind() == NodeKind::NilNode || is_zero_literal(node)
    }

    /// RuboCop's `float?`.
    fn is_float(node: &Node<'_>) -> bool {
        match node.kind() {
            NodeKind::FloatNode => true,
            NodeKind::CallNode => {
                node.as_call_node().is_some_and(|call| Self::is_float_send(&call))
            }
            NodeKind::ParenthesesNode => node
                .as_parentheses_node()
                .and_then(|p| p.body())
                .and_then(|b| b.as_statements_node())
                .and_then(|s| s.body().first())
                .is_some_and(|first| Self::is_float(&first)),
            _ => false,
        }
    }

    /// RuboCop's `float_send?`.
    fn is_float_send(call: &CallNode<'_>) -> bool {
        let name = call.name();
        let name = name.as_slice();
        if is_arithmetic_operator(name) {
            let receiver_is_float =
                call.receiver().is_some_and(|receiver| Self::is_float(&receiver));
            let first_argument_is_float =
                first_argument(call).is_some_and(|arg| Self::is_float(&arg));
            return receiver_is_float || first_argument_is_float;
        }
        if is_float_returning_method(name) {
            return true;
        }
        let Some(receiver) = call.receiver() else { return false };
        if receiver.kind() != NodeKind::FloatNode {
            return false;
        }
        is_float_instance_method(name) || is_numeric_returning_method(call)
    }

    /// RuboCop's `on_send`/`on_csend` (Prism uses one `CallNode` kind for
    /// both).
    fn check_send(call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let name = call.name();
        let name = name.as_slice();
        if !matches!(name, b"==" | b"!=" | b"eql?" | b"equal?") {
            return;
        }
        let Some(argument) = call.arguments() else { return };
        if argument.arguments().len() != 1 {
            return;
        }
        let rhs = argument.arguments().first().expect("checked len == 1");
        let lhs = call.receiver();

        if lhs.as_ref().is_some_and(Self::is_literal_safe) || Self::is_literal_safe(&rhs) {
            return;
        }

        let lhs_is_float = lhs.as_ref().is_some_and(Self::is_float);
        if !(lhs_is_float || Self::is_float(&rhs)) {
            return;
        }

        let message = if name == b"!=" { MSG_INEQUALITY } else { MSG_EQUALITY };
        ctx.report(&Self::META, call.as_node().span(), message);
    }

    /// RuboCop's `on_case`.
    fn check_case(node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(case) = node.as_case_node() else { return };
        for branch in &case.conditions() {
            let Some(when) = branch.as_when_node() else { continue };
            for condition in &when.conditions() {
                if !Self::is_float(&condition) || Self::is_literal_safe(&condition) {
                    continue;
                }
                ctx.report(&Self::META, condition.span(), MSG_CASE);
            }
        }
    }
}

/// RuboCop's `numeric_returning_method?`.
fn is_numeric_returning_method(call: &CallNode<'_>) -> bool {
    let Some(receiver) = call.receiver() else { return false };
    let name = call.name();
    match name.as_slice() {
        b"angle" | b"arg" | b"phase" => {
            let source = String::from_utf8_lossy(receiver.location().as_slice());
            source
                .trim()
                .replace('_', "")
                .parse::<f64>()
                .is_ok_and(|value| value.is_sign_negative() && value != 0.0)
        }
        b"ceil" | b"floor" | b"round" | b"truncate" => {
            first_argument(call).is_some_and(|precision| {
                precision.kind() == NodeKind::IntegerNode
                    && String::from_utf8_lossy(precision.location().as_slice())
                        .trim()
                        .replace('_', "")
                        .parse::<i64>()
                        .is_ok_and(|value| value > 0)
            })
        }
        _ => false,
    }
}

/// `call.first_argument`: the first positional argument, if any.
fn first_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    call.arguments().and_then(|args| args.arguments().first())
}

/// RuboCop-AST's `(node.numeric_type? && node.value.zero?)`, restricted to
/// the literal kinds `numeric_type?` covers.
fn is_zero_literal(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::IntegerNode => {
            node.as_integer_node().is_some_and(|n| TryInto::<i32>::try_into(n.value()) == Ok(0))
        }
        NodeKind::FloatNode => node.as_float_node().is_some_and(|n| n.value() == 0.0),
        NodeKind::RationalNode => node
            .as_rational_node()
            .is_some_and(|n| TryInto::<i32>::try_into(n.numerator()) == Ok(0)),
        NodeKind::ImaginaryNode => {
            node.as_imaginary_node().is_some_and(|n| is_zero_literal(&n.numeric()))
        }
        _ => false,
    }
}

impl Rule for FloatComparison {
    const META: RuleMeta = RuleMeta {
        name: "Lint/FloatComparison",
        department: Department::Lint,
        summary: "Checks for the presence of precise comparison of floating point numbers.",
        explanation: "\
Floating point values are inherently inaccurate, and comparing them for exact equality
is almost never the desired semantics. Comparison via the `==`/`!=` operators checks
floating-point value representation to be exactly the same, which is very unlikely
if you perform any arithmetic operations involving precision loss.

```ruby
# bad
x == 0.1
x != 0.1

# bad
case value
when 1.0
  foo
when 2.0
  bar
end

# good - using BigDecimal
x.to_d == 0.1.to_d

# good - comparing against zero
x == 0.0
x != 0.0

# good
(x - 0.1).abs < Float::EPSILON

# good
tolerance = 0.0001
(x - 0.1).abs < tolerance

# good - comparing against nil
Float(x, exception: false) == nil

# good - using epsilon comparison in case expression
case
when (value - 1.0).abs < Float::EPSILON
  foo
when (value - 2.0).abs < Float::EPSILON
  bar
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::CaseNode],
        config: &[],
        blind_spots: "\
`float_send?`'s instance-method branch (upstream's `node.receiver&.float_type?`) only
recognizes a receiver that is *literally* a float literal, not one that merely produces a
float, so e.g. `x.to_f.abs == 0.1` is not flagged via that path -- an upstream limitation,
not one introduced by this port. `numeric_returning_method?`'s `angle`/`arg`/`phase` and
`ceil`/`floor`/`round`/`truncate` branches parse the receiver's/argument's own source text
as a number, exactly as upstream's `Float(...)`/`Integer(...)` calls on `node.source`.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                if let Some(call) = node.as_call_node() {
                    Self::check_send(&call, ctx);
                }
            }
            NodeKind::CaseNode => Self::check_case(node, ctx),
            _ => {}
        }
    }
}
