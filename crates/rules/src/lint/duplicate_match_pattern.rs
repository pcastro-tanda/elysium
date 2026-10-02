//! `Lint/DuplicateMatchPattern`, ported from RuboCop's
//! `lib/rubocop/cop/lint/duplicate_match_pattern.rb`.
//!
//! # Approach
//!
//! Upstream's `in_pattern` node carries its guard as a separate child
//! (`pattern.parent.children[1]`), but Prism folds a guard onto the
//! pattern itself: `InNode#pattern` is an `IfNode`/`UnlessNode` wrapping
//! the real pattern in its `statements` and the guard condition in its
//! `predicate` (see the porting kit's "Prism note"). [`real_pattern`]
//! unwraps that back into `(real pattern, guard source text)`, matching
//! `pattern_identity`'s own `pattern.parent.children[1]` lookup plus
//! `guard.source` (reconstructed as the `if`/`unless` keyword through the
//! end of the guard condition, which is exactly what whitequark's
//! `if_guard`/`unless_guard` node's own source covers).
//!
//! Upstream's `minimum_target_ruby_version 2.7` is dead here: Prism only
//! ever parses as 3.3+, and `case`/`in` pattern matching syntax itself
//! requires 2.7, so the cop is unconditionally enabled (see
//! `lint/ambiguous_regexp_literal.rs` for the same kind of inert
//! `TargetRubyVersion` branch).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Duplicate `in` pattern detected.";

/// Do not repeat patterns in `in` keywords.
#[derive(Debug, Clone)]
pub struct DuplicateMatchPattern;

impl Rule for DuplicateMatchPattern {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DuplicateMatchPattern",
        department: Department::Lint,
        summary: "Do not repeat patterns in `in` keywords.",
        explanation: "\
Checks that there are no repeated patterns used in `in` keywords.

```ruby
# bad
case x
in 'first'
  do_something
in 'first'
  do_something_else
end

# good
case x
in 'first'
  do_something
in 'second'
  do_something_else
end

# bad - repeated alternate patterns with the same conditions don't depend on the order
case x
in 0 | 1
  first_method
in 1 | 0
  second_method
end

# good
case x
in 0 | 1
  first_method
in 2 | 3
  second_method
end

# bad - repeated hash patterns with the same conditions don't depend on the order
case x
in foo: a, bar: b
  first_method
in bar: b, foo: a
  second_method
end

# good
case x
in foo: a, bar: b
  first_method
in bar: b, baz: c
  second_method
end

# bad - repeated array patterns with elements in the same order
case x
in [foo, bar]
  first_method
in [foo, bar]
  second_method
end

# good
case x
in [foo, bar]
  first_method
in [bar, foo]
  second_method
end

# bad - repeated the same patterns and guard conditions
case x
in foo if bar
  first_method
in foo if bar
  second_method
end

# good
case x
in foo if bar
  first_method
in foo if baz
  second_method
end
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CaseMatchNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(case_match) = node.as_case_match_node() else { return };
        let mut seen: Vec<Vec<u8>> = Vec::new();
        for condition in &case_match.conditions() {
            let Some(in_node) = condition.as_in_node() else { continue };
            let (pattern, guard) = real_pattern(ctx, in_node.pattern());
            let identity = pattern_identity(ctx, &pattern, guard.as_deref());
            if seen.contains(&identity) {
                ctx.report(&Self::META, pattern.span(), MSG);
            } else {
                seen.push(identity);
            }
        }
    }
}

/// Unwraps a Prism guard-wrapped pattern (see the module doc) into the real
/// pattern node and, if there was a guard, its source text (the `if`/
/// `unless` keyword through the end of the guard condition -- whitequark's
/// `if_guard`/`unless_guard`'s own `source`).
fn real_pattern<'pr>(ctx: &Context<'_>, pattern: Node<'pr>) -> (Node<'pr>, Option<Vec<u8>>) {
    match pattern.kind() {
        NodeKind::IfNode => {
            let if_node = pattern.as_if_node().expect("kind matched");
            if let (Some(kw), Some(statements)) = (if_node.if_keyword_loc(), if_node.statements()) {
                if let Some(inner) = statements.body().iter().next() {
                    let guard_span = Span::new(kw.span().start, if_node.predicate().span().end);
                    return (inner, Some(ctx.text(guard_span).to_vec()));
                }
            }
            (pattern, None)
        }
        NodeKind::UnlessNode => {
            let unless_node = pattern.as_unless_node().expect("kind matched");
            if let Some(statements) = unless_node.statements() {
                if let Some(inner) = statements.body().iter().next() {
                    let guard_span = Span::new(
                        unless_node.keyword_loc().span().start,
                        unless_node.predicate().span().end,
                    );
                    return (inner, Some(ctx.text(guard_span).to_vec()));
                }
            }
            (pattern, None)
        }
        _ => (pattern, None),
    }
}

/// RuboCop's `pattern_identity`: the pattern's own identity text (for
/// `hash_pattern`/`match_alt`, its immediate children's source texts
/// sorted and joined, order-independent; otherwise its own raw source),
/// plus the guard's source text if present.
fn pattern_identity(ctx: &Context<'_>, pattern: &Node<'_>, guard: Option<&[u8]>) -> Vec<u8> {
    let mut identity = match pattern.kind() {
        NodeKind::HashPatternNode => {
            let hp = pattern.as_hash_pattern_node().expect("kind matched");
            let mut parts: Vec<&[u8]> = hp.elements().iter().map(|e| ctx.text(e.span())).collect();
            if let Some(rest) = hp.rest() {
                parts.push(ctx.text(rest.span()));
            }
            parts.sort_unstable();
            join_sorted(&parts)
        }
        NodeKind::AlternationPatternNode => {
            let alt = pattern.as_alternation_pattern_node().expect("kind matched");
            let mut parts = [ctx.text(alt.left().span()), ctx.text(alt.right().span())];
            parts.sort_unstable();
            join_sorted(&parts)
        }
        _ => ctx.text(pattern.span()).to_vec(),
    };
    if let Some(guard) = guard {
        identity.extend_from_slice(guard);
    }
    identity
}

/// Joins already-sorted byte-slice parts with a `\0` separator (an
/// arbitrary but collision-free delimiter for this internal identity key;
/// Ruby's own `Array#to_s` representation is not reproduced since nothing
/// reads this value besides the equality check it's built for).
fn join_sorted(parts: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            out.push(0);
        }
        out.extend_from_slice(part);
    }
    out
}
