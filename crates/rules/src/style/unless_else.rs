//! `Style/UnlessElse`, ported from RuboCop's
//! `lib/rubocop/cop/style/unless_else.rb`.
//!
//! Upstream's `ignore_node`/`part_of_ignored_node?` dance (skip
//! autocorrecting a nested `unless`/`else` once its enclosing one has
//! already swapped it into place) is reproduced with a plain `Vec<Span>` of
//! already-corrected node spans: this rule is cloned per file and visited
//! top-down, so an outer offense is always recorded before its nested
//! children are visited.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Do not use `unless` with `else`. Rewrite these with the positive case first.";

/// Do not use unless with else. Rewrite these with the positive case first.
#[derive(Debug, Clone, Default)]
pub struct UnlessElse {
    /// Spans of `unless`/`else` nodes already corrected; a node nested
    /// inside one of these is `part_of_ignored_node?` and only gets its
    /// offense reported, never a fix.
    corrected: Vec<Span>,
}

impl Rule for UnlessElse {
    const META: RuleMeta = RuleMeta {
        name: "Style/UnlessElse",
        department: Department::Style,
        summary: "Do not use unless with else. Rewrite these with the positive case first.",
        explanation: "Looks for `unless` expressions with `else` clauses.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::UnlessNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let unless_node = node.as_unless_node().expect("kind matched");
        let Some(else_node) = unless_node.else_clause() else { return };
        let node_span = node.span();

        let part_of_ignored_node =
            self.corrected.iter().any(|s| s.start <= node_span.start && node_span.end <= s.end);
        if part_of_ignored_node {
            ctx.report(&Self::META, node_span, MSG);
            return;
        }

        // `range_between_condition_and_else`: from the end of the explicit
        // `then` keyword if present, else the end of the condition, up to
        // the start of `else`.
        let body_start = unless_node
            .then_keyword_loc()
            .map_or_else(|| unless_node.predicate().span().end, |loc| loc.span().end);
        let else_keyword_span = else_node.else_keyword_loc().span();
        let body_range = Span::new(body_start, else_keyword_span.start);

        // `range_between_else_and_end`: from the end of `else` to the start
        // of `end`.
        let end_keyword_span =
            unless_node.end_keyword_loc().expect("unless with else always has end").span();
        let else_range = Span::new(else_keyword_span.end, end_keyword_span.start);

        let body_text = ctx.text(body_range).to_vec();
        let else_text = ctx.text(else_range).to_vec();

        let edits = vec![
            Edit::replace(unless_node.keyword_loc().span(), b"if".to_vec()),
            Edit::replace(body_range, else_text),
            Edit::replace(else_range, body_text),
        ];
        ctx.report_with_fix(
            &Self::META,
            node_span,
            MSG,
            Fix { applicability: Applicability::Safe, edits },
        );

        self.corrected.push(node_span);
    }
}
