//! `Lint/EachWithObjectArgument`, ported from RuboCop's
//! `lib/rubocop/cop/lint/each_with_object_argument.rb` plus rubocop-ast's
//! `Node#immutable_literal?` (`lib/rubocop/ast/node.rb`), which the cop
//! relies on to decide whether the argument makes sense as the object
//! `each_with_object` builds up.
//!
//! # `each_with_object?` node pattern
//!
//! Upstream's node pattern `(call _ :each_with_object $_)` only matches a
//! `each_with_object` dispatch carrying *exactly one* argument -- a second
//! argument, or none at all, makes the pattern itself fail to match, so
//! `on_send` returns without ever consulting `immutable_literal?`. This port
//! mirrors that by requiring `CallNode::arguments` to hold exactly one node.
//!
//! # Node-kind mirror of `IMMUTABLE_LITERALS`
//!
//! rubocop-ast's `IMMUTABLE_LITERALS` is `LITERALS - MUTABLE_LITERALS`:
//! `int`, `float`, `sym`, `dsym`, `true`, `false`, `nil`, `complex`,
//! `rational` (and `regopt`, which can never appear as a bare argument
//! expression). Prism's equivalents are `IntegerNode`, `FloatNode`,
//! `SymbolNode`, `InterpolatedSymbolNode`, `TrueNode`, `FalseNode`,
//! `NilNode`, `ImaginaryNode` and `RationalNode`.
//!
//! # Offense span
//!
//! Upstream reports on the whole `send` node, which -- since whitequark
//! attaches a block as a separate wrapping `block` node -- never includes
//! `{ |e, a| ... }`. [`ruby_ast::ext::call_span_excluding_block`] reproduces
//! that against Prism's `CallNode`, whose own span otherwise extends
//! through its attached block.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "The argument to each_with_object cannot be immutable.";

/// `Lint::EachWithObjectArgument`.
#[derive(Debug, Clone)]
pub struct EachWithObjectArgument;

impl Rule for EachWithObjectArgument {
    const META: RuleMeta = RuleMeta {
        name: "Lint/EachWithObjectArgument",
        department: Department::Lint,
        summary: "Checks if `each_with_object` is called with an immutable argument.",
        explanation: "\
Checks if each_with_object is called with an immutable
argument. Since the argument is the object that the given block shall
make calls on to build something based on the enumerable that
each_with_object iterates over, an immutable argument makes no sense.
It's definitely a bug.

```ruby
# bad
sum = numbers.each_with_object(0) { |e, a| a += e }

# good
num = 0
sum = numbers.each_with_object(num) { |e, a| a += e }
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Only literal argument expressions are recognized (rubocop-ast's
`IMMUTABLE_LITERALS`: integers, floats, symbols, `true`/`false`/`nil`,
complex and rational literals). A variable or method call that is known at
runtime to hold one of these values, or a frozen mutable literal (e.g.
`''.freeze`), is not flagged -- matching upstream, which never traces
values back to their definitions.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"each_with_object" {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let args = args.arguments();
        if args.len() != 1 {
            return;
        }
        let arg = args.first().expect("len checked above");
        if !is_immutable_literal(&arg) {
            return;
        }
        ctx.report(&Self::META, ext::call_span_excluding_block(&call), MSG);
    }
}

/// rubocop-ast's `Node#immutable_literal?`: `IMMUTABLE_LITERALS.include?(type)`,
/// where `IMMUTABLE_LITERALS` is `LITERALS - MUTABLE_LITERALS`.
fn is_immutable_literal(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
    )
}
