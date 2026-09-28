//! `Lint/RequireParentheses`, ported from RuboCop's
//! `lib/rubocop/cop/lint/require_parentheses.rb`.
//!
//! # `operator_keyword?`
//!
//! Upstream's `check_predicate`/`check_ternary` both gate on
//! `predicate.operator_keyword?`/`ternary.condition.operator_keyword?`,
//! which (confusingly) checks only the AST node *type* (`:and`/`:or`) --
//! true for `&&`/`||` *and* for the keyword spellings `and`/`or`, which
//! whitequark's parser gives the identical node type. This still excludes
//! the keyword spellings in practice: `and`/`or` bind far more loosely than
//! a bare (unparenthesized) method call's argument list, so `day.is?
//! 'tuesday' and month == :jan` parses with the `and` *outside* the `send`
//! node entirely (as `(and (send day :is? (str "tuesday")) ...)`) -- the
//! `send` node's own last argument is just the string, never the `and`
//! node -- while `&&`/`||` bind tighter than the argument list and so *do*
//! end up as the last argument. Prism's grammar has the same precedence, so
//! [`is_boolean_operator`] checks node kind only (`AndNode`/`OrNode`,
//! covering both the symbolic and keyword spellings, exactly like
//! upstream's type-only check), and the keyword form is excluded here for
//! the identical structural reason rather than by any explicit check.
//!
//! # Ternary shape
//!
//! Prism represents a ternary (`a ? b : c`) as an ordinary [`IfNode`] with
//! no `if_keyword_loc` (see e.g. `Style::SoleNestedConditional`'s module
//! doc for the same shape), reached here as a `CallNode`'s first argument
//! whenever the whole conditional is itself unparenthesized;
//! [`is_ternary`] mirrors upstream's `node.if_type? && node.ternary?` with
//! that single check.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::{CallNode, IfNode};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Use parentheses in the method call to avoid confusion about precedence.";

/// RuboCop-AST's `OPERATOR_METHODS`, needed only for the `[]` member: `node.method?(:[])`
/// in `check_ternary`.
const INDEX_METHOD: &[u8] = b"[]";

/// RuboCop-AST's `MethodIdentifierPredicates#predicate_method?`.
fn is_predicate_method(name: &[u8]) -> bool {
    name.ends_with(b"?")
}

/// RuboCop-AST's `MethodIdentifierPredicates#assignment_method?`.
fn is_assignment_method(name: &[u8]) -> bool {
    name.ends_with(b"=") && !matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=")
}

/// A `CallNode`'s own `parenthesized?`: its argument list is wrapped in `(...)`.
fn is_parenthesized(call: &CallNode<'_>) -> bool {
    call.opening_loc().is_some_and(|o| o.as_slice() == b"(")
}

/// RuboCop's `node.first_argument.if_type? && node.first_argument.ternary?` (see the module
/// doc for the Prism shape this checks).
fn is_ternary<'pr>(node: &Node<'pr>) -> Option<IfNode<'pr>> {
    let if_node = node.as_if_node()?;
    if_node.if_keyword_loc().is_none().then_some(if_node)
}

/// RuboCop's `predicate.operator_keyword?`/`ternary.condition.operator_keyword?` (see the
/// module doc).
fn is_boolean_operator(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::AndNode | NodeKind::OrNode)
}

/// Checks for expressions where there is a call to a predicate method with at least one
/// argument, where no parentheses are used around the parameter list, and a boolean
/// operator, && or ||, is used in the last argument.
#[derive(Debug, Clone, Default)]
pub struct RequireParentheses;

impl Rule for RequireParentheses {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RequireParentheses",
        department: Department::Lint,
        summary: "Checks for expressions where there is a call to a predicate \
                  method with at least one argument, where no parentheses are \
                  used around the parameter list, and a boolean operator, && or \
                  ||, is used in the last argument.",
        explanation: "\
Checks for expressions where there is a call to a predicate
method with at least one argument, where no parentheses are used around
the parameter list, and a boolean operator, && or ||, is used in the
last argument.

The idea behind warning for these constructs is that the user might
be under the impression that the return value from the method call is
an operand of &&/||.

```ruby
# bad
if day.is? :tuesday && month == :jan
  # ...
end

# good
if day.is?(:tuesday) && month == :jan
  # ...
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Structural equality checks fall back on this codebase's usual exact source-text comparison, but
this cop performs none itself; nothing else is known to diverge from upstream.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let Some(args) = call.arguments() else { return };
        let arguments = args.arguments();
        if arguments.is_empty() || is_parenthesized(&call) {
            return;
        }
        let first_argument = arguments.first().expect("checked non-empty above");
        if let Some(ternary) = is_ternary(&first_argument) {
            check_ternary(&ternary, &call, ctx);
        } else if is_predicate_method(call.name().as_slice()) {
            let last_argument = arguments.last().expect("checked non-empty above");
            check_predicate(&last_argument, &call, ctx);
        }
    }
}

/// RuboCop's `check_ternary`.
fn check_ternary(ternary: &IfNode<'_>, node: &CallNode<'_>, ctx: &mut Context<'_>) {
    if node.name().as_slice() == INDEX_METHOD
        || is_assignment_method(node.name().as_slice())
        || !is_boolean_operator(&ternary.predicate())
    {
        return;
    }
    let range = Span::new(node.as_node().span().start, ternary.predicate().span().end);
    ctx.report(&RequireParentheses::META, range, MSG);
}

/// RuboCop's `check_predicate`.
fn check_predicate(predicate: &Node<'_>, node: &CallNode<'_>, ctx: &mut Context<'_>) {
    if !is_boolean_operator(predicate) {
        return;
    }
    ctx.report(&RequireParentheses::META, node.as_node().span(), MSG);
}
