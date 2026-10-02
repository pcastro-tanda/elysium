//! `Lint/AmbiguousRange`, ported from RuboCop's
//! `lib/rubocop/cop/lint/ambiguous_range.rb` plus the `RationalLiteral`
//! mixin it includes.
//!
//! `literal?`/`basic_literal?` (rubocop-ast's `LITERALS`/`BASIC_LITERALS`
//! sets) are restated here directly against the Prism node kinds they cover
//! -- `basic_literal?` is the non-composite subset (no `dstr`/`xstr`/`dsym`/
//! `array`/`hash`/`irange`/`erange`/`regexp` equivalents).
//! `RationalLiteral#rational_literal?`'s `(send (int _) :/ (rational _))`
//! pattern (`1/10r`) is a `CallNode` whose receiver is an `IntegerNode` and
//! whose sole argument is a `RationalNode`.
//!
//! `unary_operation?` is `CallNode::message_loc` starting at the same offset
//! as the call's own span (no receiver text precedes the operator) *and*
//! the method name being one of rubocop-ast's `OPERATOR_METHODS` -- see
//! `Layout::SpaceAroundOperators`' `is_regular_operator` for the same
//! position check (that cop does not need the operator-method-name half,
//! since it only ever visits nodes already known to carry an operator-shaped
//! name).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "Wrap complex range boundaries with parentheses to avoid ambiguity.";

/// rubocop-ast's `MethodIdentifierPredicates::OPERATOR_METHODS`.
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

/// rubocop-ast's `BASIC_LITERALS` (`LITERALS - COMPOSITE_LITERALS`).
fn is_basic_literal(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
    )
}

/// rubocop-ast's `LITERALS`: [`is_basic_literal`] plus the composite kinds
/// (`dstr`/`xstr`/`dsym`/`array`/`hash`/`irange`/`erange`/`regexp`).
fn is_literal(kind: NodeKind) -> bool {
    is_basic_literal(kind)
        || matches!(
            kind,
            NodeKind::InterpolatedStringNode
                | NodeKind::XStringNode
                | NodeKind::InterpolatedXStringNode
                | NodeKind::InterpolatedSymbolNode
                | NodeKind::ArrayNode
                | NodeKind::HashNode
                | NodeKind::RegularExpressionNode
                | NodeKind::InterpolatedRegularExpressionNode
                | NodeKind::RangeNode
        )
}

fn is_variable(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableReadNode
            | NodeKind::InstanceVariableReadNode
            | NodeKind::ClassVariableReadNode
            | NodeKind::GlobalVariableReadNode
            | NodeKind::ItLocalVariableReadNode
    )
}

/// `RationalLiteral#rational_literal?`: `1/10r`.
fn is_rational_literal(call: &CallNode<'_>) -> bool {
    call.name().as_slice() == b"/"
        && call.receiver().is_some_and(|r| r.kind() == NodeKind::IntegerNode)
        && call.arguments().is_some_and(|args| {
            let list = args.arguments();
            list.len() == 1 && list.last().is_some_and(|arg| arg.kind() == NodeKind::RationalNode)
        })
}

/// `MethodDispatchNode#unary_operation?`.
fn is_unary_operation(call: &CallNode<'_>) -> bool {
    let Some(message_loc) = call.message_loc() else { return false };
    OPERATOR_METHODS.contains(&call.name().as_slice())
        && message_loc.span().start == call.as_node().span().start
}

/// `AmbiguousRange#acceptable_call?`.
fn acceptable_call(call: &CallNode<'_>, allow_chain: bool) -> bool {
    if is_unary_operation(call) {
        return true;
    }
    if call.receiver().is_some_and(|r| is_basic_literal(r.kind())) {
        return false;
    }
    let name = call.name().as_slice();
    if OPERATOR_METHODS.contains(&name) && name != b"[]" {
        return false;
    }
    allow_chain || call.receiver().is_none()
}

/// `AmbiguousRange#acceptable?`.
fn is_acceptable_boundary(node: &Node<'_>, allow_chain: bool) -> bool {
    let kind = node.kind();
    if kind == NodeKind::ParenthesesNode
        || is_literal(kind)
        || is_variable(kind)
        || kind == NodeKind::ConstantReadNode
        || kind == NodeKind::ConstantPathNode
        || kind == NodeKind::SelfNode
    {
        return true;
    }
    let Some(call) = node.as_call_node() else { return false };
    is_rational_literal(&call) || acceptable_call(&call, allow_chain)
}

/// Checks for ambiguous ranges.
#[derive(Debug, Clone)]
pub struct AmbiguousRange {
    require_parentheses_for_method_chains: bool,
}

impl Rule for AmbiguousRange {
    const META: RuleMeta = RuleMeta {
        name: "Lint/AmbiguousRange",
        department: Department::Lint,
        summary: "Checks for ranges with ambiguous boundaries.",
        explanation: "\
Checks for ambiguous ranges.

Ranges have quite low precedence, which leads to unexpected behavior when \
using a range with other operators. This cop avoids that by making ranges \
explicit by requiring parenthesis around complex range boundaries (anything \
that is not a literal: numerics, strings, symbols, etc.).

This cop can be configured with `RequireParenthesesForMethodChains` in order \
to specify whether method chains (including `self.foo`) should be wrapped \
in parens by this cop.

NOTE: Regardless of this configuration, if a method receiver is a basic \
literal value, it will be wrapped in order to prevent the ambiguity of \
`1..2.to_a`.

```ruby
# bad
x || 1..2
x - 1..2
(x || 1..2)
x || 1..y || 2
1..2.to_a

# good, unambiguous
1..2
'a'..'z'
:bar..:baz
MyClass::MIN..MyClass::MAX
@min..@max
a..b
-a..b

# good, ambiguity removed
x || (1..2)
(x - 1)..2
(x || 1)..2
(x || 1)..(y || 2)
(1..2).to_a
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::RangeNode],
        config: &[ConfigOption {
            name: "RequireParenthesesForMethodChains",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Require parentheses for method chain boundaries (including `self.foo`).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            require_parentheses_for_method_chains: options
                .bool("RequireParenthesesForMethodChains"),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(range) = node.as_range_node() else { return };
        let allow_chain = !self.require_parentheses_for_method_chains;

        for boundary in [range.left(), range.right()].into_iter().flatten() {
            if is_acceptable_boundary(&boundary, allow_chain) {
                continue;
            }
            let span = boundary.span();
            let fix = Fix {
                applicability: Applicability::Unsafe,
                edits: vec![
                    Edit::insert(span.start, b"(".to_vec()),
                    Edit::insert(span.end, b")".to_vec()),
                ],
            };
            ctx.report_with_fix(&Self::META, span, MSG, fix);
        }
    }
}
