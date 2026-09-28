//! `Lint/RescueException`, ported from RuboCop's
//! `lib/rubocop/cop/lint/rescue_exception.rb`.
//!
//! RuboCop's whitequark-based `on_resbody(node)` fires once per `resbody`
//! child of a `:rescue` node; Prism gives each `rescue` clause its own
//! `RescueNode` kind directly (chained via `RescueNode::subsequent`, see
//! `duplicate_rescue_exception.rs`'s module doc), so subscribing straight to
//! `NodeKind::RescueNode` reaches exactly the same clauses one at a time --
//! no reconstruction of the chain is needed here since, unlike
//! `Lint/DuplicateRescueException`/`Lint/ShadowedException`, this cop never
//! looks past the clause it is currently visiting.
//!
//! `targets_exception?` is `rescue_arg_node.const_name == 'Exception'`,
//! i.e. rubocop-ast's generic `Node#const_name` applied to whatever shape
//! the exception expression happens to have (only a `(const ...)` node
//! returns non-`nil`; a splat, method call, or local variable always misses
//! and is silently skipped, matching the "does not crash" specs for
//! `rescue *ERRORS` and `rescue adapter::ParseError`). This port narrows
//! that to [`is_bare_or_toplevel_const`] before comparing
//! [`ruby_ast::ext::const_name`] against `"Exception"`: a namespaced
//! `Test::Exception` has the right short name but is never bare/toplevel,
//! so it is rejected by the shape check exactly as upstream's `const_name`
//! (which requires every namespace segment to itself already be a
//! constant) rejects it for real.
//!
//! `add_offense(node)` highlights the whole `resbody` -- its whitequark
//! source range runs from the `rescue` keyword through the last of the
//! exception list/`=>` variable, but *not* into the clause's body (see the
//! `rescue Exception\n  #do nothing\nend` spec: only the `rescue Exception`
//! line is underlined). Prism's `RescueNode::location` does not reproduce
//! this: for every clause but the last in a chain it extends through every
//! `subsequent` clause too (the same quirk documented in
//! `shadowed_exception.rs`'s module doc), so [`resbody_header_span`]
//! recomputes it from the clause's own keyword/exceptions/`=>`-variable
//! children instead, exactly like that cop's `shadowing_span`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::node::RescueNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str =
    "Avoid rescuing the `Exception` class. Perhaps you meant to rescue `StandardError`?";

/// RuboCop's `offense_range`-equivalent for a `resbody`: the `rescue`
/// keyword through the end of whichever of the `=>` variable or the last
/// exception expression is present, never into the clause's body. See the
/// module doc comment for why `RescueNode::location` cannot be reused.
fn resbody_header_span(rescue_node: &RescueNode<'_>) -> Span {
    let start = rescue_node.keyword_loc().span().start;
    let end = if let Some(reference) = rescue_node.reference() {
        reference.span().end
    } else if let Some(operator_loc) = rescue_node.operator_loc() {
        operator_loc.span().end
    } else if let Some(last_exception) = rescue_node.exceptions().last() {
        last_exception.span().end
    } else {
        rescue_node.keyword_loc().span().end
    };
    Span::new(start, end)
}

/// RuboCop's `targets_exception?`.
fn targets_exception(exception: Node<'_>) -> bool {
    is_bare_or_toplevel_const(&exception) && const_name(&exception).as_deref() == Some("Exception")
}

/// Checks for `rescue` blocks targeting the `Exception` class.
#[derive(Debug, Clone)]
pub struct RescueException;

impl Rule for RescueException {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RescueException",
        department: Department::Lint,
        summary: "Checks for `rescue` blocks targeting the `Exception` class.",
        explanation: "\
Checks for `rescue` blocks targeting the `Exception` class.

```ruby
# bad
begin
  do_something
rescue Exception
  handle_exception
end

# good
begin
  do_something
rescue ArgumentError
  handle_exception
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::RescueNode],
        config: &[],
        blind_spots: "\
Matches only `rescue`-clause exceptions whose shape is a bare or \
toplevel-qualified constant read, same as upstream's `const_name` (which \
requires every namespace segment to itself already be a constant): a \
splat (`rescue *ERRORS`), a method call, or a namespace held in a local \
variable (`rescue adapter::ParseError`) can never match, matching \
upstream's own \"does not crash\" specs for those shapes.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let resbody = node.as_rescue_node().expect("kind matched");
        if resbody.exceptions().iter().any(targets_exception) {
            ctx.report(&Self::META, resbody_header_span(&resbody), MSG);
        }
    }
}
