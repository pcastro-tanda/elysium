//! `Security/Eval`, ported from RuboCop's
//! `lib/rubocop/cop/security/eval.rb`.
//!
//! # Receiver shape
//!
//! Upstream's matcher, `(send {nil? (send nil? :binding) (const {cbase nil?}
//! :Kernel)} :eval $!str ...)`, only fires on a bare `eval(...)`, a bare,
//! argument-less `binding.eval(...)`, or a bare-or-top-level-qualified
//! `Kernel.eval(...)`; any other receiver (`foo.eval`, a `binding` call that
//! itself takes arguments, `SomeModule::Kernel.eval`, ...) never reaches the
//! cop. [`has_eval_receiver_shape`] reproduces that alternation directly on
//! `CallNode::receiver`.
//!
//! # `$!str ...` and `recursive_literal?`
//!
//! The pattern's argument slot captures the *first* argument only if it is
//! not a plain `str` node, and allows (via the trailing `...`) any number of
//! further arguments (the explicit `binding`/filename/line-number form). A
//! call with no arguments at all fails to match the pattern's minimum arity,
//! so `eval` alone is silently accepted. [`Eval::enter`] mirrors both: it
//! bails out with no offense when there is no first argument, and again when
//! that argument is a plain string literal.
//!
//! `on_send` additionally exempts a `dstr` (interpolated string) argument
//! that is [`recursive_literal?`](is_recursive_literal): every `#{...}` part
//! is itself a literal, e.g. `eval "something#{2}"`. [`is_recursive_literal`]
//! ports rubocop-ast's `Node#recursive_literal?`, restricted to the node
//! kinds that can actually appear inside an interpolated string's parts (the
//! composite literal kinds, `and`/`or`, and a `begin`-equivalent wrapping a
//! nested statement list) -- see its own doc and the `blind_spots` entry
//! below for the one upstream branch (a handful of literal-only comparison
//! operators) this does not model.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "The use of `eval` is a serious security risk.";

/// Checks for the use of `Kernel#eval` and `Binding#eval`.
#[derive(Debug, Clone)]
pub struct Eval;

impl Rule for Eval {
    const META: RuleMeta = RuleMeta {
        name: "Security/Eval",
        department: Department::Security,
        summary: "Checks for the use of `Kernel#eval` and `Binding#eval`.",
        explanation: "\
Checks for the use of `Kernel#eval` and `Binding#eval`.

```ruby
# bad

eval(something)
binding.eval(something)
Kernel.eval(something)
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Upstream's `recursive_literal?` also treats a call to one of a fixed set of
comparison-ish methods (`==`, `!=`, `<`, `>`, `<=`, `>=`, `<=>`, `*`, `!`) as
literal when its receiver and arguments are themselves all literal (e.g. an
interpolation like `\"#{1 == 1}\"`). This port only walks composite literal
node kinds, `and`/`or`, and nested statement bodies, so that one shape is
treated as non-literal (flagged) where upstream would accept it. No fixture
in this corpus exercises it.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"eval" {
            return;
        }
        if !has_eval_receiver_shape(&call) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let Some(code) = arguments.arguments().first() else { return };
        if code.kind() == NodeKind::StringNode {
            return;
        }
        if code.kind() == NodeKind::InterpolatedStringNode && is_recursive_literal(&code) {
            return;
        }
        let selector = call.message_loc().map_or_else(|| call.location().span(), |loc| loc.span());
        ctx.report(&Self::META, selector, MSG);
    }
}

/// RuboCop's receiver alternation: `{nil? (send nil? :binding) (const {cbase
/// nil?} :Kernel)}`.
fn has_eval_receiver_shape(call: &CallNode<'_>) -> bool {
    match call.receiver() {
        None => true,
        Some(receiver) => is_bare_binding_call(&receiver) || is_kernel_const(&receiver),
    }
}

/// `(send nil? :binding)`: a bare, argument-less, block-less call to
/// `binding`.
fn is_bare_binding_call(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|c| {
        c.receiver().is_none()
            && c.name().as_slice() == b"binding"
            && c.arguments().is_none()
            && c.block().is_none()
    })
}

/// `(const {cbase nil?} :Kernel)`: a bare or top-level-qualified `Kernel`
/// constant reference.
fn is_kernel_const(node: &Node<'_>) -> bool {
    ext::is_bare_or_toplevel_const(node) && ext::const_name(node).as_deref() == Some("Kernel")
}

/// rubocop-ast's `Node#recursive_literal?`, restricted to the node kinds
/// that can appear as a part of an interpolated string/symbol/regexp: the
/// basic literal kinds are trivially literal; the composite literal kinds
/// (nested `dstr`/`dsym`/array/hash/range/regexp) recurse into their own
/// children; `and`/`or` recurse into both operands; and a `#{...}`
/// interpolation's own body (an `EmbeddedStatementsNode` wrapping a
/// `StatementsNode`, or a parenthesized grouping) recurses into its
/// statement(s) -- upstream's shared `begin` node type covering both. Any
/// other kind (a variable read, a method call not covered by the
/// `blind_spots` note, ...) is not literal.
fn is_recursive_literal(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::StringNode
        | NodeKind::XStringNode
        | NodeKind::IntegerNode
        | NodeKind::FloatNode
        | NodeKind::RationalNode
        | NodeKind::ImaginaryNode
        | NodeKind::SymbolNode
        | NodeKind::TrueNode
        | NodeKind::FalseNode
        | NodeKind::NilNode
        | NodeKind::RegularExpressionNode => true,
        NodeKind::InterpolatedStringNode => node
            .as_interpolated_string_node()
            .is_some_and(|n| n.parts().iter().all(|p| is_recursive_literal(&p))),
        NodeKind::InterpolatedXStringNode => node
            .as_interpolated_x_string_node()
            .is_some_and(|n| n.parts().iter().all(|p| is_recursive_literal(&p))),
        NodeKind::InterpolatedSymbolNode => node
            .as_interpolated_symbol_node()
            .is_some_and(|n| n.parts().iter().all(|p| is_recursive_literal(&p))),
        NodeKind::InterpolatedRegularExpressionNode => node
            .as_interpolated_regular_expression_node()
            .is_some_and(|n| n.parts().iter().all(|p| is_recursive_literal(&p))),
        NodeKind::ArrayNode => node
            .as_array_node()
            .is_some_and(|n| n.elements().iter().all(|e| is_recursive_literal(&e))),
        NodeKind::HashNode => node
            .as_hash_node()
            .is_some_and(|n| n.elements().iter().all(|e| is_recursive_literal(&e))),
        NodeKind::AssocNode => node
            .as_assoc_node()
            .is_some_and(|n| is_recursive_literal(&n.key()) && is_recursive_literal(&n.value())),
        NodeKind::AssocSplatNode => node
            .as_assoc_splat_node()
            .is_some_and(|n| n.value().is_none_or(|v| is_recursive_literal(&v))),
        NodeKind::RangeNode => node.as_range_node().is_some_and(|n| {
            n.left().is_none_or(|l| is_recursive_literal(&l))
                && n.right().is_none_or(|r| is_recursive_literal(&r))
        }),
        NodeKind::AndNode => node
            .as_and_node()
            .is_some_and(|n| is_recursive_literal(&n.left()) && is_recursive_literal(&n.right())),
        NodeKind::OrNode => node
            .as_or_node()
            .is_some_and(|n| is_recursive_literal(&n.left()) && is_recursive_literal(&n.right())),
        NodeKind::EmbeddedStatementsNode => node.as_embedded_statements_node().is_some_and(|n| {
            n.statements().is_none_or(|s| s.body().iter().all(|stmt| is_recursive_literal(&stmt)))
        }),
        NodeKind::ParenthesesNode => node
            .as_parentheses_node()
            .is_some_and(|n| n.body().is_none_or(|b| is_recursive_literal(&b))),
        NodeKind::StatementsNode => node
            .as_statements_node()
            .is_some_and(|n| n.body().iter().all(|stmt| is_recursive_literal(&stmt))),
        _ => false,
    }
}
