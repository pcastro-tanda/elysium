//! `Lint/DuplicateRescueException`, ported from RuboCop's
//! `lib/rubocop/cop/lint/duplicate_rescue_exception.rb`.
//!
//! RuboCop's whitequark-based `on_rescue` fires once per `:rescue` node (the
//! whole `begin ... rescue ... rescue ... end`/`def ... rescue ... end`
//! compound), then walks `node.resbody_branches` -- the mixin's helper that
//! flattens the whitequark `resbody` chain into a plain array. Prism instead
//! links each `rescue` clause to the next with `RescueNode::subsequent`, and
//! folds a `def`'s implicit `rescue` into a `BeginNode` body exactly like an
//! explicit `begin...end` (see `shadowed_exception.rs`'s module doc for the
//! `Prism.parse` proof), so subscribing to `NodeKind::BeginNode` and reading
//! `rescue_clause` then walking `subsequent` reconstructs the same branch
//! list without a separate mixin.
//!
//! `rescue_modifier?` is whitequark's guard against `foo rescue nil` (which
//! also parses to a `:rescue` node there); Prism gives the modifier form its
//! own `RescueModifierNode` kind, so this rule simply never subscribes to it
//! and the guard has no Prism equivalent to port.
//!
//! Upstream's `previous.add?(exception)` is a `Set` of whitequark AST nodes,
//! deduplicated by `Parser::AST::Node#hash`/`#eql?` -- structural equality
//! independent of source location. This port approximates that with the
//! exception expression's raw source text (already how `evaluate_exceptions`
//! in `shadowed_exception.rs` treats the identical `RescueNode::exceptions`
//! shape), which is exact for every plain constant, constant path, and splat
//! (`*ERRORS`) exercised by the upstream spec; a documented blind spot for
//! two differently-formatted-but-structurally-equal expressions (e.g.
//! `Foo::Bar` vs `Foo :: Bar`) is noted below.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::RescueNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use std::collections::HashSet;

/// RuboCop's `MSG`.
const MSG: &str = "Duplicate `rescue` exception detected.";

/// Checks that there are no repeated exceptions used in `rescue` expressions.
#[derive(Debug, Clone)]
pub struct DuplicateRescueException;

impl Rule for DuplicateRescueException {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DuplicateRescueException",
        department: Department::Lint,
        summary: "Checks that there are no repeated exceptions used in `rescue` expressions.",
        explanation: "\
Checks that there are no repeated exceptions
used in `rescue` expressions.

```ruby
# bad
begin
  something
rescue FirstException
  handle_exception
rescue FirstException
  handle_other_exception
end

# good
begin
  something
rescue FirstException
  handle_exception
rescue SecondException
  handle_other_exception
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::BeginNode],
        config: &[],
        blind_spots: "\
Duplicate detection compares each exception expression's raw source text
rather than RuboCop's true structural `Node#==`; two genuinely equal
expressions written with different incidental formatting (extra whitespace
around `::`, for instance) are treated as distinct and not flagged.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let begin = node.as_begin_node().expect("kind matched");
        let Some(head) = begin.rescue_clause() else { return };

        let mut previous: HashSet<&[u8]> = HashSet::new();
        let mut current = Some(head);
        while let Some(resbody) = current {
            check_resbody(&resbody, ctx, &mut previous);
            current = resbody.subsequent();
        }
    }
}

/// `node.resbody_branches.each_with_object(Set.new) do |resbody, previous|
/// ... end`'s per-`resbody` body: each of the clause's own exceptions is
/// checked against (and, if new, added to) the running set.
fn check_resbody<'src>(
    resbody: &RescueNode<'_>,
    ctx: &mut Context<'src>,
    previous: &mut HashSet<&'src [u8]>,
) {
    for exception in &resbody.exceptions() {
        let text = ctx.text(exception.span());
        if !previous.insert(text) {
            ctx.report(&DuplicateRescueException::META, exception.span(), MSG);
        }
    }
}
