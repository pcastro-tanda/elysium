//! `Style/NestedTernaryOperator`, ported from RuboCop's
//! `lib/rubocop/cop/style/nested_ternary_operator.rb`.
//!
//! A ternary is Prism's [`ruby_ast::node::IfNode`] with no `if_keyword_loc`;
//! its `?` is `then_keyword_loc`, its `:` is the `else_keyword_loc` of the
//! `ElseNode` in `subsequent` (never an `elsif` `IfNode`, since ternary
//! syntax admits no `elsif`).
//!
//! Upstream's `on_if` fires once per `:if`-type node during a single tree
//! walk (so once for the outer ternary *and* once again for every nested
//! ternary reached independently, since those are `IfNode`s too), each call
//! separately re-scanning `each_descendant(:if).select(&:ternary?)` for
//! nested ternaries anywhere in its own subtree. `Base#add_offense` only
//! keeps the *first* report at a given range (`current_offense_locations`,
//! a per-investigation `Set`), so a nested ternary already reported from an
//! ancestor's scan is silently skipped when a closer ancestor's own `on_if`
//! rediscovers it -- [`Self::reported`] mirrors that Set. `ignore_node`
//! similarly means only the first offense mapped to a given *outer* ternary
//! actually runs `autocorrect`; further offenses mapped to the same outer
//! node (multiple nested ternaries under one outer ternary) are
//! offense-only -- [`Self::ignored`] mirrors that.
//!
//! A single correction only ever rewrites the *outermost* ternary passed to
//! `on_if`, leaving any ternary nested in its `else`-branch text untouched
//! (still literally nested); RuboCop's own `expect_correction` (and real
//! `-A`) re-investigates the corrected source and repeats until stable
//! (`RuboCop::Runner::MAX_ITERATIONS`), converting one further nesting level
//! per pass. That is reproduced here by the fixture harness's own fix-loop,
//! not by this rule, which -- like upstream -- only ever performs one
//! outer-ternary rewrite per pass.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};
use std::collections::HashSet;

/// RuboCop's `MSG`.
const MSG: &str = "Ternary operators must not be nested. Prefer `if` or `else` constructs instead.";

/// Checks for nested ternary op expressions.
#[derive(Debug, Clone, Default)]
pub struct NestedTernaryOperator {
    /// RuboCop's `Base#current_offense_locations`: spans already reported
    /// in this pass, so a nested ternary rediscovered from an ancestor
    /// ternary's own scan is not reported twice.
    reported: HashSet<Span>,
    /// RuboCop's `ignore_node`/`part_of_ignored_node?`: outer ternary spans
    /// already autocorrected once in this pass.
    ignored: HashSet<Span>,
}

impl Rule for NestedTernaryOperator {
    const META: RuleMeta = RuleMeta {
        name: "Style/NestedTernaryOperator",
        department: Department::Style,
        summary: "Checks for nested ternary op expressions.",
        explanation: "\
Checks for nested ternary op expressions.

```ruby
# bad
a ? (b ? b1 : b2) : a2

# good
if a
  b ? b1 : b2
else
  a2
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(if_node) = node.as_if_node() else { return };
        // RuboCop's `node.ternary?` (`loc?(:question)`, i.e. no `if`/`elsif` keyword).
        if if_node.if_keyword_loc().is_some() {
            return;
        }
        let outer_span = node.span();
        each_descendant(node, &mut |child| {
            let Some(child_if) = child.as_if_node() else { return };
            if child_if.if_keyword_loc().is_some() {
                return;
            }
            let span = child.span();
            if !self.reported.insert(span) {
                // Already reported from an ancestor ternary's own scan.
                return;
            }
            if self.ignored.insert(outer_span) {
                // First offense mapped to this outer ternary: correct it.
                let edits = autocorrect_edits(&if_node, ctx);
                ctx.report_with_fix(
                    &Self::META,
                    span,
                    MSG,
                    Fix { applicability: Applicability::Safe, edits },
                );
            } else {
                ctx.report(&Self::META, span, MSG);
            }
        });
    }
}

/// RuboCop's `autocorrect`.
fn autocorrect_edits(if_node: &ruby_ast::node::IfNode<'_>, ctx: &Context<'_>) -> Vec<Edit> {
    let outer_span = if_node.as_node().span();
    let question_span = if_node.then_keyword_loc().expect("ternary always has a `?`").span();
    let else_node = if_node
        .subsequent()
        .and_then(|s| s.as_else_node())
        .expect("ternary always has a `:` branch");
    let colon_span = else_node.else_keyword_loc().span();
    let if_branch = if_node.statements().expect("ternary always has a then-branch");
    let if_branch_span = if_branch.as_node().span();

    vec![
        Edit::replace(
            ctx.with_surrounding_space(question_span, Side::Both, true, true),
            b"\n".to_vec(),
        ),
        Edit::replace(
            ctx.with_surrounding_space(colon_span, Side::Both, true, true),
            b"\nelse\n".to_vec(),
        ),
        Edit::replace(if_branch_span, remove_parentheses(ctx.text(if_branch_span))),
        Edit::insert(outer_span.start, b"if ".to_vec()),
        Edit::insert(outer_span.end, b"\nend".to_vec()),
    ]
}

/// RuboCop's `remove_parentheses`.
fn remove_parentheses(source: &[u8]) -> Vec<u8> {
    if source.first() == Some(&b'(') && source.last() == Some(&b')') {
        source[1..source.len() - 1].to_vec()
    } else {
        source.to_vec()
    }
}
