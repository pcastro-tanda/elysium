//! `Lint/ParenthesesAsGroupedExpression`, ported from RuboCop's
//! `lib/rubocop/cop/lint/parentheses_as_grouped_expression.rb`.
//!
//! Upstream's `valid_context?`/`valid_first_argument?` guard against several
//! shapes (`node.first_argument.any_block_type?`, `chained_calls?`'s
//! `first_argument.call_type?`, and `valid_first_argument?`'s
//! `operator_keyword?`/`hash_type?`/`ternary_expression?`/`compound_range?`)
//! that all inspect `node.first_argument` itself -- but that same node is
//! only ever reached once `node.first_argument.parenthesized_call?` (whose
//! Prism equivalent is [`ruby_ast::node::Node::as_parentheses_node`]) has
//! already proven it. Real parens around a bare argument always parse as
//! their own wrapper node (whitequark's `begin`, Prism's
//! [`ruby_ast::node::ParenthesesNode`]) rather than leaving the wrapped
//! node's own type (`block`/`call`/`if`/`hash`/`range`) exposed at the top,
//! so none of those six checks can ever be true in real Ruby source; this
//! port omits them (a faithful no-op, not a behavior change).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop-AST's `MethodIdentifierPredicates::OPERATOR_METHODS`.
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

/// Checks for method calls with a space before the opening parenthesis.
#[derive(Debug, Clone)]
pub struct ParenthesesAsGroupedExpression;

impl Rule for ParenthesesAsGroupedExpression {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ParenthesesAsGroupedExpression",
        department: Department::Lint,
        summary: "Checks for method calls with a space before the opening parenthesis.",
        explanation: "\
Checks for space between the name of a called method and a left
parenthesis.

```ruby
# bad
do_something (foo)

# good
do_something(foo)
do_something (2 + 3) * 4
do_something (foo * bar).baz
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Upstream's `valid_context?`/`valid_first_argument?`/`chained_calls?` guard
against several `node.first_argument` shapes (a block, a call chain, an
operator keyword, a hash, a ternary, a parenthesized range) that can never
actually occur once `parenthesized_call?` has already proven the argument is
a real-parens wrapper node; see the module doc for why. Omitted as dead code,
not a behavior change.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // A call that already has its own arguments-parens (`opening_loc`)
        // is a normal parenthesized call, never the ambiguous "space then
        // parens" shape this cop targets (RuboCop's regex-based
        // `spaces_before_left_parenthesis` only matches when the `(` right
        // after the method name -- reached only via whitespace -- belongs to
        // the argument itself, not to the call's own parameter list).
        if call.opening_loc().is_some() {
            return;
        }
        let Some(argument) = single_argument(&call) else { return };
        let Some(paren) = argument.as_parentheses_node() else { return };
        if is_operator_method(&call) || is_setter_method(&call) {
            return;
        }

        let Some(message_loc) = call.message_loc() else { return };
        let space_start = message_loc.span().end;
        let space_end = paren.as_node().span().start;
        if space_end <= space_start
            || !ctx.text(Span::new(space_start, space_end)).iter().all(u8::is_ascii_whitespace)
        {
            return;
        }
        let space_span = Span::new(space_start, space_end);

        let argument_source =
            String::from_utf8_lossy(ctx.text(paren.as_node().span())).into_owned();
        let message = format!("`{argument_source}` interpreted as grouped expression.");

        ctx.report_with_fix(
            &Self::META,
            space_span,
            message,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(space_span)] },
        );
    }
}

/// The call's single positional argument, if it has exactly one.
fn single_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let arguments = call.arguments()?.arguments();
    if arguments.len() == 1 {
        arguments.first()
    } else {
        None
    }
}

/// RuboCop-AST's `MethodIdentifierPredicates#operator_method?`.
fn is_operator_method(call: &CallNode<'_>) -> bool {
    OPERATOR_METHODS.contains(&call.name().as_slice())
}

/// RuboCop-AST's `MethodDispatchNode#setter_method?` (`loc?(:operator)`):
/// an attribute-writer call like `a.b = x`, whose `equal_loc` is Prism's
/// equivalent of whitequark's `loc.operator`.
fn is_setter_method(call: &CallNode<'_>) -> bool {
    call.equal_loc().is_some()
}
