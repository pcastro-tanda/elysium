//! `Style/EachForSimpleLoop`, ported from RuboCop's
//! `lib/rubocop/cop/style/each_for_simple_loop.rb`.
//!
//! Upstream's `offending?` gate, `node.arguments.empty?`, calls
//! `BlockNode#arguments` (rubocop-ast's name for a block's own *parameter*
//! list, not the wrapped call's arguments) -- so it actually requires the
//! block to take no parameters at all, unconditionally; the
//! `each_range_with_zero_origin?`/`each_range_without_block_argument?` OR is
//! then always satisfied once that gate passes (both patterns' `args`
//! sub-pattern accepts an already-empty args node), leaving only the shared
//! [`each_range`]-equivalent shape to check: a `CallNode` named `each`,
//! attached `BlockNode`, whose receiver is a `ParenthesesNode` wrapping a
//! single-statement `StatementsNode` holding a `RangeNode` with
//! integer-literal endpoints on both sides.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::BlockNode;
use ruby_ast::{Node, NodeKind};

const MSG: &str = "Use `Integer#times` for a simple loop which iterates a fixed number of times.";

/// An `IntegerNode`'s value, if it fits `i32`.
fn integer_value(node: &Node<'_>) -> Option<i32> {
    node.as_integer_node().and_then(|n| n.value().try_into().ok())
}

/// Upstream's `(args)` pattern: a block declaring no parameters at all (not
/// even shadow args), matching both `{ }`/`do end` (no `BlockParametersNode`)
/// and the degenerate `{ || }` (a `BlockParametersNode` with nothing in it).
fn block_has_no_params(block: &BlockNode<'_>) -> bool {
    let Some(params) = block.parameters() else { return true };
    let Some(block_params) = params.as_block_parameters_node() else { return false };
    if !block_params.locals().is_empty() {
        return false;
    }
    block_params.parameters().is_none_or(|p| {
        p.requireds().is_empty()
            && p.optionals().is_empty()
            && p.rest().is_none()
            && p.posts().is_empty()
            && p.keywords().is_empty()
            && p.keyword_rest().is_none()
            && p.block().is_none()
    })
}

/// Use `Integer#times` for a simple loop which iterates a fixed number of times.
#[derive(Debug, Clone)]
pub struct EachForSimpleLoop;

impl Rule for EachForSimpleLoop {
    const META: RuleMeta = RuleMeta {
        name: "Style/EachForSimpleLoop",
        department: Department::Style,
        summary: "Use `Integer#times` for a simple loop which iterates a fixed number of times.",
        explanation: "\
Checks for loops which iterate a constant number of times,
using a `Range` literal and `#each`. This can be done more readably using
`Integer#times`.

This check only applies if the block takes no parameters.

```ruby
# bad
(1..5).each { }

# good
5.times { }

# bad
(0...10).each {}

# good
10.times {}
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"each" {
            return;
        }
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        if !block_has_no_params(&block) {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(parens) = receiver.as_parentheses_node() else { return };
        let Some(body) = parens.body() else { return };
        let Some(stmts) = body.as_statements_node() else { return };
        let statements = stmts.body();
        if statements.len() != 1 {
            return;
        }
        let Some(inner) = statements.iter().next() else { return };
        let Some(range) = inner.as_range_node() else { return };
        let Some(left) = range.left() else { return };
        let Some(right) = range.right() else { return };
        let Some(min) = integer_value(&left) else { return };
        let Some(max) = integer_value(&right) else { return };

        let times = if range.is_exclude_end() { max - min } else { max - min + 1 };

        let span = call_span_excluding_block(&call);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, format!("{times}.times").into_bytes())],
            },
        );
    }
}
