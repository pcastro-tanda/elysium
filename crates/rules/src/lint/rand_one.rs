//! `Lint/RandOne`, ported from RuboCop's `lib/rubocop/cop/lint/rand_one.rb`.
//!
//! Upstream's `rand_one?` node-pattern matches a `send` node's fixed shape --
//! receiver, `:rand` selector, exactly one argument that is an `int` or
//! `float` literal equal to `1` or `-1` -- and does not look at whether that
//! send is itself wrapped in a block node, so `rand(1) { }` still matches.
//! Prism instead folds an attached block directly into the `CallNode` (its
//! own `block` field) rather than wrapping the call in a separate node, but
//! since [`is_rand_one`] never inspects [`CallNode::block`] either, the
//! behavior is the same: this port matches on receiver/name/argument shape
//! alone, ignoring any attached block.
//!
//! A negative numeric literal (`-1`, `-1.0`) is, in Prism, folded into the
//! `IntegerNode`/`FloatNode` itself (its span covers the leading `-`, and
//! `Integer::try_into::<i32>` applies the sign) rather than represented as a
//! unary-minus call wrapping a positive literal, so no separate unary-minus
//! case is needed here.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{ext, Node, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "`{}` always returns `0`. Perhaps you meant `rand(2)` or `rand`?";

/// Checks for `rand(1)` calls. Such calls always return `0`.
#[derive(Debug, Clone)]
pub struct RandOne;

impl Rule for RandOne {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RandOne",
        department: Department::Lint,
        summary: "Checks for `rand(1)` calls.",
        explanation: "\
Checks for `rand(1)` calls.
Such calls always return `0`.

```ruby
# bad

rand 1
Kernel.rand(-1)
rand 1.0
rand(-1.0)

# good

0 # just use 0 instead
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        if !is_rand_one(&call) {
            return;
        }

        let span = ext::call_span_excluding_block(&call);
        let method = String::from_utf8_lossy(ctx.text(span));
        let message = MSG.replacen("{}", &method, 1);
        ctx.report(&Self::META, span, message);
    }
}

/// RuboCop's `rand_one?` node-pattern matcher:
/// `(send {(const {nil? cbase} :Kernel) nil?} :rand {(int {-1 1}) (float {-1.0 1.0})})`.
fn is_rand_one(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() != b"rand" {
        return false;
    }
    if !is_rand_one_receiver(call.receiver()) {
        return false;
    }
    let Some(arguments) = call.arguments() else {
        return false;
    };
    let args = arguments.arguments();
    let mut iter = (&args).into_iter();
    let Some(only_arg) = iter.next() else {
        return false;
    };
    if iter.next().is_some() {
        return false;
    }
    is_rand_one_literal(&only_arg)
}

/// The receiver half of `rand_one?`'s pattern: either absent, or a bare
/// (`Kernel`) or top-level-qualified (`::Kernel`) reference to the `Kernel`
/// constant.
fn is_rand_one_receiver(receiver: Option<Node<'_>>) -> bool {
    match receiver {
        None => true,
        Some(recv) => {
            ext::is_bare_or_toplevel_const(&recv)
                && ext::const_name(&recv).as_deref() == Some("Kernel")
        }
    }
}

/// The argument half of `rand_one?`'s pattern: an `int` or `float` literal
/// whose value is exactly `1` or `-1`.
fn is_rand_one_literal(node: &Node<'_>) -> bool {
    match node {
        Node::IntegerNode { .. } => {
            let int_node = node.as_integer_node().expect("kind matched");
            matches!(TryInto::<i32>::try_into(int_node.value()), Ok(1 | -1))
        }
        Node::FloatNode { .. } => {
            let float_node = node.as_float_node().expect("kind matched");
            let value = float_node.value();
            value.to_bits() == 1.0f64.to_bits() || value.to_bits() == (-1.0f64).to_bits()
        }
        _ => false,
    }
}
