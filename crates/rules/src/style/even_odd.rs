//! `Style/EvenOdd`, ported from RuboCop's
//! `lib/rubocop/cop/style/even_odd.rb`.
//!
//! Upstream's `even_odd_candidate?` node matcher:
//!
//! ```text
//! (send
//!   {(send $_ :% (int 2))
//!    (begin (send $_ :% (int 2)))}
//!   ${:== :!=}
//!   (int ${0 1}))
//! ```
//!
//! The `begin` alternative is whitequark's node for a parenthesized single
//! statement; in Prism that is a `ParenthesesNode` wrapping a
//! `StatementsNode` with one element, so [`base_number_candidate`] unwraps
//! that shape before checking for the `%` call.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop-AST's `MethodIdentifierPredicates::OPERATOR_METHODS`.
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

/// RuboCop's `operator_method?`, restricted to the `receiver_source` use
/// site which additionally excludes `[]` (`!node.method?(:[])`).
fn is_wrapping_operator(name: &[u8]) -> bool {
    OPERATOR_METHODS.contains(&name) && name != b"[]"
}

/// The sole argument of a call, if it has exactly one.
fn only_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let args = call.arguments()?;
    let list = args.arguments();
    if list.len() != 1 {
        return None;
    }
    list.iter().next()
}

/// An `IntegerNode` argument's value, if it fits `i32`.
fn integer_value(node: &Node<'_>) -> Option<i32> {
    node.as_integer_node().and_then(|n| n.value().try_into().ok())
}

/// Matches `(send $_ :% (int 2))`, returning the `%` call's receiver
/// (upstream's `base_number`).
fn match_percent_two<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    if call.name().as_slice() != b"%" {
        return None;
    }
    if integer_value(&only_argument(call)?) != Some(2) {
        return None;
    }
    call.receiver()
}

/// Matches either alternative of the candidate's receiver: a bare `_ % 2`
/// call, or one parenthesized as a single statement (whitequark's `begin`).
fn base_number_candidate<'pr>(receiver: &Node<'pr>) -> Option<Node<'pr>> {
    if let Some(call) = receiver.as_call_node() {
        return match_percent_two(&call);
    }
    let parens = receiver.as_parentheses_node()?;
    let body = parens.body()?;
    let stmts = body.as_statements_node()?;
    let list = stmts.body();
    if list.len() != 1 {
        return None;
    }
    let inner = list.iter().next()?;
    match_percent_two(&inner.as_call_node()?)
}

/// Upstream's `replacement_method`.
fn replacement_method(arg: i32, method: &[u8]) -> &'static str {
    match (arg, method == b"==") {
        (0, true) | (1, false) => "even",
        _ => "odd",
    }
}

/// Upstream's `receiver_source`: a binary/unary operator receiver binds
/// looser than the appended method call, so it must be parenthesized;
/// `[]` is explicitly excluded since indexing binds tightly enough.
fn receiver_source(ctx: &Context<'_>, node: &Node<'_>) -> String {
    let text = String::from_utf8_lossy(ctx.text(node.span())).into_owned();
    if node.as_call_node().is_some_and(|call| is_wrapping_operator(call.name().as_slice())) {
        format!("({text})")
    } else {
        text
    }
}

/// Favor the use of `Integer#even?` && `Integer#odd?`.
#[derive(Debug, Clone)]
pub struct EvenOdd;

impl Rule for EvenOdd {
    const META: RuleMeta = RuleMeta {
        name: "Style/EvenOdd",
        department: Department::Style,
        summary: "Favor the use of `Integer#even?` && `Integer#odd?`.",
        explanation: "\
Checks for places where `Integer#even?` or `Integer#odd?` can be used.

```ruby
# bad
if x % 2 == 0
end

# good
if x.even?
end
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
        let method = call.name().as_slice();
        if method != b"==" && method != b"!=" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(base_number) = base_number_candidate(&receiver) else { return };
        let Some(arg) = only_argument(&call) else { return };
        let Some(arg_value @ (0 | 1)) = integer_value(&arg) else { return };

        let replacement = replacement_method(arg_value, method);
        let message = format!("Replace with `Integer#{replacement}?`.");
        let correction = format!("{}.{replacement}?", receiver_source(ctx, &base_number));
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(node.span(), correction.into_bytes())],
            },
        );
    }
}
