//! `Lint/DuplicateCaseCondition`, ported from RuboCop's
//! `lib/rubocop/cop/lint/duplicate_case_condition.rb`.
//!
//! Upstream compares each `when` condition against every condition already
//! seen in the same `case` via a `Set` keyed by `Parser::AST::Node#eql?`/
//! `#hash` (structural, type-and-children equality, insensitive to
//! incidental formatting). This port instead compares conditions by their
//! exact source text, matching this codebase's established approximation
//! for the same problem in `Style/RedundantCondition` and
//! `Lint/SelfAssignment`: two conditions that are semantically/structurally
//! identical but written with different incidental formatting (extra
//! parens, different whitespace, `'x'` vs `"x"`) are treated as distinct.
//! Every fixture condition is written identically at each repeated site, so
//! this is exact for everything tested.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Duplicate `when` condition detected.";

/// `Lint::DuplicateCaseCondition`.
#[derive(Debug, Clone)]
pub struct DuplicateCaseCondition;

impl Rule for DuplicateCaseCondition {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DuplicateCaseCondition",
        department: Department::Lint,
        summary: "Checks that there are no repeated conditions used in case 'when' expressions.",
        explanation: "\
Checks that there are no repeated conditions used in case 'when' expressions.

```ruby
# bad
case x
when 'first'
  do_something
when 'first'
  do_something_else
end

# good
case x
when 'first'
  do_something
when 'second'
  do_something_else
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CaseNode],
        config: &[],
        blind_spots: "\
Conditions are compared by exact source text rather than RuboCop's true structural `Node#==`
(mirroring this codebase's established approximation in `Style/RedundantCondition` and
`Lint/SelfAssignment`); two conditions that are structurally identical but written with different
incidental formatting (extra parens, different whitespace, `'x'` vs `\"x\"`) are not flagged as
duplicates. Only plain `case`/`when` is checked, matching upstream's `on_case`; `case`/`in` pattern
matching (`CaseMatchNode`) is a distinct node kind upstream never subscribes to.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(case_node) = node.as_case_node() else { return };

        // RuboCop's `case_node.when_branches.each_with_object(Set.new)`.
        let mut previous: Vec<&[u8]> = Vec::new();
        for when_node in &case_node.conditions() {
            let Some(when_node) = when_node.as_when_node() else { continue };
            for condition in &when_node.conditions() {
                let text = ctx.text(condition.span());
                if previous.contains(&text) {
                    ctx.report(&Self::META, condition.span(), MSG);
                } else {
                    previous.push(text);
                }
            }
        }
    }
}
