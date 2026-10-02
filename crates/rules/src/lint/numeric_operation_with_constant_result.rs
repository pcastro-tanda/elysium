//! `Lint/NumericOperationWithConstantResult`, ported from RuboCop's
//! `lib/rubocop/cop/lint/numeric_operation_with_constant_result.rb`.
//!
//! # Node pattern capture semantics
//!
//! Upstream's matchers capture *values*, not nodes: `(call nil? $_lhs)`
//! binds `$_lhs` to the bare call's own method-name symbol (a `call` node's
//! second child), and `({int | call nil?} $_rhs)` binds `$_rhs` to either
//! the `int` node's own integer value or the alternative bare call's
//! method-name symbol -- confirmed directly against RuboCop 1.91.0, since
//! this is easy to misread as capturing the whole sub-node. `constant_result?`
//! then compares `rhs == lhs` as a plain name/value comparison. This port
//! mirrors that directly: [`bare_call_name`] extracts the symbol a bare,
//! receiver-less, argument-less call or (for the abbreviated-assignment
//! path) a local-variable read stands for.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// Upstream's `MSG`.
const MSG: &str = "Numeric operation with a constant result detected.";

/// Checks for numeric operations with constant results.
#[derive(Debug, Clone)]
pub struct NumericOperationWithConstantResult;

impl Rule for NumericOperationWithConstantResult {
    const META: RuleMeta = RuleMeta {
        name: "Lint/NumericOperationWithConstantResult",
        department: Department::Lint,
        summary: "Checks for numeric operations with constant results.",
        explanation: "\
Certain numeric operations have a constant result, usually 0 or 1.
Multiplying a number by 0 will always return 0.
Dividing a number by itself or raising it to the power of 0 will always return 1.
As such, they can be replaced with that result.
These are probably leftover from debugging, or are mistakes.
Other numeric operations that are similarly leftover from debugging or mistakes
are handled by `Lint/UselessNumericOperation`.

NOTE: This cop doesn't detect offenses for the `-` and `%` operator because it
can't determine the type of `x`. If `x` is an `Array` or `String`, it doesn't perform
a numeric operation.

@safety
  This cop is unsafe because the autocorrection drops the operands, which
  discards any side effects of evaluating them and can change behavior when
  the result is not actually constant. For example, `x / x` raises
  `ZeroDivisionError` when `x` is `0`, and returns `Float::NAN` (not `1`)
  when `x` is `0.0`; replacing it with `1` silences that.

```ruby
# bad
x * 0

# good
0

# bad
x *= 0

# good
x = 0

# bad
x / x
x ** 0

# good
1

# bad
x /= x
x **= 0

# good
x = 1
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::LocalVariableOperatorWriteNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => check_send(node, ctx),
            NodeKind::LocalVariableOperatorWriteNode => check_op_asgn(node, ctx),
            _ => {}
        }
    }
}

/// One side of a matched operation, the way upstream's node pattern
/// captures it: an integer value, or the name a bare call/local-variable
/// read stands for.
enum Side {
    Int(i32),
    Name(Vec<u8>),
}

/// The symbol a bare, receiver-less, argument-less, block-less call stands
/// for -- upstream's `(call nil? $_lhs)`/`(call nil? $_rhs)`.
fn bare_call_name(node: &Node<'_>) -> Option<Vec<u8>> {
    let call = node.as_call_node()?;
    if call.receiver().is_some() || call.arguments().is_some() || call.block().is_some() {
        return None;
    }
    Some(call.name().as_slice().to_vec())
}

/// Upstream's `operation_with_constant_result?`.
fn check_send(node: &Node<'_>, ctx: &mut Context<'_>) {
    let Some(call) = node.as_call_node() else { return };
    if call.is_safe_navigation() {
        return;
    }
    let op = call.name();
    let op = op.as_slice();
    if !matches!(op, b"*" | b"/" | b"**") {
        return;
    }
    let Some(receiver) = call.receiver() else { return };
    let Some(lhs) = bare_call_name(&receiver) else { return };
    let Some(args) = call.arguments() else { return };
    let args = args.arguments();
    if args.len() != 1 {
        return;
    }
    let arg = args.first().expect("len checked");
    let rhs = if let Some(int) = arg.as_integer_node() {
        let Ok(value) = TryInto::<i32>::try_into(int.value()) else { return };
        Side::Int(value)
    } else if let Some(name) = bare_call_name(&arg) {
        Side::Name(name)
    } else {
        return;
    };
    let Some(result) = constant_result(&lhs, op, &rhs) else { return };
    let span = node.span();
    let fix = Fix {
        applicability: Applicability::Unsafe,
        edits: vec![Edit::replace(span, result.to_string().into_bytes())],
    };
    ctx.report_with_fix(&NumericOperationWithConstantResult::META, span, MSG, fix);
}

/// Upstream's `abbreviated_assignment_with_constant_result?`.
fn check_op_asgn(node: &Node<'_>, ctx: &mut Context<'_>) {
    let Some(n) = node.as_local_variable_operator_write_node() else { return };
    let op = n.binary_operator();
    let op = op.as_slice();
    if !matches!(op, b"*" | b"/" | b"**") {
        return;
    }
    let lhs = n.name().as_slice().to_vec();
    let value = n.value();
    let rhs = if let Some(int) = value.as_integer_node() {
        let Ok(v) = TryInto::<i32>::try_into(int.value()) else { return };
        Side::Int(v)
    } else if let Some(lvar) = value.as_local_variable_read_node() {
        Side::Name(lvar.name().as_slice().to_vec())
    } else {
        return;
    };
    let Some(result) = constant_result(&lhs, op, &rhs) else { return };
    let span = node.span();
    let replacement = format!("{} = {result}", String::from_utf8_lossy(&lhs));
    let fix = Fix {
        applicability: Applicability::Unsafe,
        edits: vec![Edit::replace(span, replacement.into_bytes())],
    };
    ctx.report_with_fix(&NumericOperationWithConstantResult::META, span, MSG, fix);
}

/// Upstream's `constant_result?`.
fn constant_result(lhs: &[u8], operation: &[u8], rhs: &Side) -> Option<i32> {
    match rhs {
        Side::Int(0) => match operation {
            b"*" => Some(0),
            b"**" => Some(1),
            _ => None,
        },
        Side::Name(name) if name == lhs => {
            if operation == b"/" {
                Some(1)
            } else {
                None
            }
        }
        _ => None,
    }
}
