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
//! `add_offense(node)` highlights the whole `resbody`, i.e. its whitequark
//! `loc.expression`, which the parser gem's `rescue_body_map` builds as
//! `keyword.join(compstmt || then || exc_var || exc_list || keyword)`: the
//! `rescue` keyword through the end of the clause's *body* when it has one
//! (an empty or comment-only body falls back to the `then`/`=>`-variable/
//! exception-list tail, so the `rescue Exception\n  #do nothing\nend` spec
//! underlines only the `rescue Exception` line). Prism's
//! `RescueNode::location` does not reproduce this: for every clause but the
//! last in a chain it extends through every `subsequent` clause too (the
//! same quirk documented in `shadowed_exception.rs`'s module doc), so
//! [`resbody_span`] recomputes it from the clause's own children, copied
//! privately from `Lint/SuppressedException`'s helper of the same name
//! (including its `;`-terminator handling -- see that module's doc).

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

/// RuboCop's `offense_range`-equivalent for a `resbody` (whitequark's
/// `loc.expression`), reconstructed from its own keyword/statements/
/// `then`/`=>`-variable/exception-list children. See the module doc
/// comment for why `RescueNode::location` cannot be reused.
fn resbody_span(ctx: &Context<'_>, rescue_node: &RescueNode<'_>) -> Span {
    let start = rescue_node.keyword_loc().span().start;
    let end = if let Some(statements) = rescue_node.statements() {
        statements.location().span().end
    } else if let Some(then_keyword) = rescue_node.then_keyword_loc() {
        then_keyword.span().end
    } else if let Some(reference) = rescue_node.reference() {
        reference.span().end
    } else if let Some(last_exception) = rescue_node.exceptions().last() {
        last_exception.span().end
    } else {
        rescue_node.keyword_loc().span().end
    };
    let end = if rescue_node.statements().is_none()
        && rescue_node.then_keyword_loc().is_none()
        && rescue_node.reference().is_none()
    {
        extend_through_semicolon(ctx, end)
    } else {
        end
    };
    Span::new(start, end)
}

/// See `Lint/SuppressedException`'s module doc ("A body-less `resbody`
/// terminated by `;` instead of `then`"): scans forward from a body-less
/// resbody's end, over horizontal whitespace only, for an immediate `;` and
/// extends the span past it when found.
fn extend_through_semicolon(ctx: &Context<'_>, end: u32) -> u32 {
    let line_col = ctx.line_col(end);
    let line_text = ctx.line_text(line_col.line);
    let col = line_col.column as usize;
    let Some(rest) = line_text.get(col..) else { return end };
    let ws = rest.iter().take_while(|&&b| b == b' ' || b == b'\t').count();
    if rest.get(ws) == Some(&b';') {
        end + u32::try_from(ws + 1).expect("offset exceeds u32")
    } else {
        end
    }
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
        stability: Stability::Stable,
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
            ctx.report(&Self::META, resbody_span(ctx, &resbody), MSG);
        }
    }
}
