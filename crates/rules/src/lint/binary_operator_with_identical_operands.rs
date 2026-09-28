//! `Lint/BinaryOperatorWithIdenticalOperands`, ported from RuboCop's
//! `lib/rubocop/cop/lint/binary_operator_with_identical_operands.rb`.
//!
//! # Two node shapes
//!
//! Upstream splits detection across `on_send` (real `send` nodes spelled
//! with one of `RESTRICT_ON_SEND`'s operator names) and `on_and`/`on_or`
//! (Ruby's `&&`/`||`/`and`/`or` keywords, which whitequark's parser --
//! and Prism -- give their own node kind rather than routing through
//! `send`, since those spellings are not overridable method names).
//! `RESTRICT_ON_SEND` also lists `&&`/`||`, but those spellings can never
//! actually reach a `send` node, so this port only subscribes to the
//! eleven names that can: `== != === <=> =~ > >= < <= | ^`.
//!
//! # `binary_operation?`
//!
//! `node.binary_operation?` (`MethodDispatchNode#binary_operation?`) is
//! `operator_method? && loc.expression.begin_pos != selector.begin_pos`:
//! true whenever an operator-named call has an explicit receiver written
//! before it (`a == b`, `a.==(b)`), false for a receiverless/unary
//! spelling. Every name this cop subscribes to is an infix binary
//! operator with no unary reading, so a `CallNode` of one of those names
//! always has an explicit receiver in practice; this port checks
//! `receiver().is_some()` directly rather than reconstructing the
//! position comparison, matching the "does not crash on operator without
//! any argument" spec (`foo.*`, whose name isn't even in the restricted
//! set, so it never reaches this code at all).
//!
//! # `on_csend`
//!
//! The cop defines no `on_csend`, so a safe-navigation spelling
//! (`a&.==(b)`) is never checked upstream even though it uses the same
//! `RESTRICT_ON_SEND`-filtered method name. Prism represents both as the
//! same `CallNode` kind, distinguished only by an `is_safe_navigation`
//! flag, so this port checks that flag explicitly to reproduce the same
//! gap.
//!
//! # Structural equality
//!
//! `node.receiver == node.first_argument` and `node.lhs == node.rhs` are
//! RuboCop's generic `Parser::AST::Node#==` (type-and-children structural
//! equality, ignoring source position). This port approximates both with
//! exact source-text comparison instead, matching this codebase's
//! established approximation for the same problem in `Lint::SelfAssignment`
//! and `Lint::DuplicateElsifCondition`: every fixture operand is a bare
//! send chain, variable, or literal written identically both times it
//! repeats, so this is exact for everything tested; two genuinely equal
//! expressions written with different incidental formatting (extra
//! parens, different whitespace) would be treated as unequal.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `RESTRICT_ON_SEND` filtered down to the names that can ever
/// reach a `CallNode` (`&&`/`||` are handled separately, as `AndNode`/
/// `OrNode`; see the module doc).
const OPERATOR_NAMES: &[&[u8]] =
    &[b"==", b"!=", b"===", b"<=>", b"=~", b">", b">=", b"<", b"<=", b"|", b"^"];

/// Builds RuboCop's `MSG`, `%<op>s` filled in with the operator's own
/// spelling (its method name for a `CallNode`, or its exact source text --
/// `&&`, `||`, `and`, or `or` -- for an `AndNode`/`OrNode`).
fn message(op: &[u8]) -> String {
    format!("Binary operator `{}` has identical operands.", String::from_utf8_lossy(op))
}

/// Checks for places where binary operator has identical operands.
#[derive(Debug, Clone, Default)]
pub struct BinaryOperatorWithIdenticalOperands;

impl Rule for BinaryOperatorWithIdenticalOperands {
    const META: RuleMeta = RuleMeta {
        name: "Lint/BinaryOperatorWithIdenticalOperands",
        department: Department::Lint,
        summary: "Checks for places where binary operator has identical operands.",
        explanation: "\
Checks for places where binary operator has identical operands.

It covers comparison operators: `==`, `===`, `=~`, `>`, `>=`, `<`, `<=`;
bitwise operators: `|`, `^`, `&`;
boolean operators: `&&`, `||`
and \"spaceship\" operator - `<=>`.

Simple arithmetic operations are allowed by this cop: `+`, `*`, `**`, `<<` and `>>`.
Although these can be rewritten in a different way, it should not be necessary to
do so. Operations such as `-` or `/` where the result will always be the same
(`x - x` will always be 0; `x / x` will always be 1) are offenses, but these
are covered by `Lint/NumericOperationWithConstantResult` instead.

```ruby
# bad
x.top >= x.top

if a.x != 0 && a.x != 0
  do_something
end

def child?
  left_child || left_child
end

# good
x + x
1 << 1
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode, NodeKind::AndNode, NodeKind::OrNode],
        config: &[],
        blind_spots: "\
This cop is unsafe as it does not consider side effects when calling methods and thus can
generate false positives (e.g. `wr.take_char == '\\0' && wr.take_char == '\\0'`); elysium does not
model side effects either, so this is inherited rather than newly introduced. Operand equality is
approximated by exact source-text comparison rather than RuboCop's true structural `Node#==`: a
genuinely equal expression written with different incidental formatting is treated as unequal.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                if let Some(call) = node.as_call_node() {
                    check_call(&call, ctx);
                }
            }
            NodeKind::AndNode => {
                if let Some(and) = node.as_and_node() {
                    let (left, right, op) = (and.left(), and.right(), and.operator_loc());
                    if ctx.text(left.span()) == ctx.text(right.span()) {
                        ctx.report(&Self::META, node.span(), message(ctx.text(op.span())));
                    }
                }
            }
            NodeKind::OrNode => {
                if let Some(or) = node.as_or_node() {
                    let (left, right, op) = (or.left(), or.right(), or.operator_loc());
                    if ctx.text(left.span()) == ctx.text(right.span()) {
                        ctx.report(&Self::META, node.span(), message(ctx.text(op.span())));
                    }
                }
            }
            _ => {}
        }
    }
}

/// RuboCop's `on_send`: `return unless node.binary_operation?` (approximated
/// by an explicit, non-safe-navigation receiver; see the module doc) then
/// `return unless node.receiver == node.first_argument`.
fn check_call(call: &CallNode<'_>, ctx: &mut Context<'_>) {
    let name = call.name();
    let name = name.as_slice();
    if !OPERATOR_NAMES.contains(&name) {
        return;
    }
    if call.is_safe_navigation() {
        return;
    }
    let Some(receiver) = call.receiver() else { return };
    let Some(first_argument) = call.arguments().and_then(|args| args.arguments().iter().next())
    else {
        return;
    };
    if ctx.text(receiver.span()) == ctx.text(first_argument.span()) {
        ctx.report(
            &BinaryOperatorWithIdenticalOperands::META,
            call.as_node().span(),
            message(name),
        );
    }
}
