//! `Style/MultilineIfThen`, ported from RuboCop's
//! `lib/rubocop/cop/style/multiline_if_then.rb`.
//!
//! # Node shapes
//!
//! `unless` is Prism's own [`UnlessNode`], never chained; `if`/`elsif` are
//! both [`IfNode`], an `elsif` reached through [`IfNode::subsequent`] and
//! told apart from a genuine `if` only by [`IfNode::if_keyword_loc`] reading
//! `"elsif"`. A ternary has `if_keyword_loc` `None` and is skipped (matching
//! upstream's `OnNormalIfUnless` mixin, which also skips modifier form --
//! here that falls out for free, since a modifier `if`/`unless` never has a
//! `then_keyword_loc` at all).
//!
//! # The `own_then?` quirk
//!
//! Upstream's `own_then?` guards against a real parser quirk: an `elsif`
//! with no `then` of its own can be handed the *previous* branch's `then`
//! location instead of `None`. This reproduces in Prism too -- confirmed by
//! parsing `"if a then b\nelsif c\n  d\nend"` and observing the `elsif`
//! link's `then_keyword_loc` pointing at the first branch's `then`, well
//! before the `elsif` keyword itself. The guard is reproduced exactly:
//! `node.span().contains(then_kw.span())`.
//!
//! # `multiline?`
//!
//! Upstream's `node.multiline?` is checked against each `if`/`elsif` node's
//! *own* whitequark span, which (for a non-terminal `elsif`) recurses
//! through everything it owns down to, but excluding, the shared terminal
//! `end`. Prism's raw `node.span()` instead always extends through that
//! shared `end`. The two agree everywhere this cop can actually tell them
//! apart: whenever `then` is present with a body on a different line, both
//! spans already cross at least one newline; the only single-line-vs-multi
//! disagreement would need an empty branch, and an empty `then`-branch node
//! (`if cond then end`) is single-line under both spans alike. So the raw
//! [`ruby_source::Span`] is used directly, no reconstruction needed.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{IfNode, StatementsNode, UnlessNode};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::{Side, Span};

/// RuboCop's `MSG`, formatted with the offending keyword (`if`/`elsif`/
/// `unless`).
fn message(keyword: &str) -> String {
    format!("Do not use `then` for multi-line `{keyword}`.")
}

/// Checks for uses of the `then` keyword in multi-line `if`/`unless`
/// statements.
#[derive(Debug, Clone)]
pub struct MultilineIfThen;

impl MultilineIfThen {
    /// RuboCop's `on_normal_if_unless`/`non_modifier_then?`, shared between
    /// `if`/`elsif` (`node.keyword` is `"if"`/`"elsif"`, read off
    /// `if_keyword_loc`) and `unless` (always `"unless"`).
    fn check(
        ctx: &mut Context<'_>,
        keyword: &str,
        own_span: Span,
        then_kw: Option<Span>,
        statements: Option<StatementsNode<'_>>,
    ) {
        let Some(then_span) = then_kw else { return };
        if ctx.text(then_span) != b"then" {
            return;
        }
        // `own_then?`: reject a `then` location that Prism handed down from
        // an earlier branch (see the module doc).
        if !own_span.contains(then_span) {
            return;
        }
        // `node.multiline?`.
        if ctx.is_single_line(own_span) {
            return;
        }
        let then_line = ctx.line_col(then_span.start).line;
        let branch_line = statements.map(|s| ctx.line_col(s.location().span().start).line);
        if branch_line == Some(then_line) {
            return;
        }
        let removal = ctx.with_surrounding_space(then_span, Side::Left, true, false);
        ctx.report_with_fix(
            &<Self as Rule>::META,
            then_span,
            message(keyword),
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(removal)] },
        );
    }

    /// A ternary (`if_keyword_loc` absent) is skipped outright; otherwise
    /// dispatches with `"if"`/`"elsif"` read off `if_keyword_loc`'s text.
    fn check_if(ctx: &mut Context<'_>, node: &IfNode<'_>) {
        let Some(if_kw) = node.if_keyword_loc() else { return };
        let keyword = String::from_utf8_lossy(ctx.text(if_kw.span())).into_owned();
        Self::check(
            ctx,
            &keyword,
            node.location().span(),
            node.then_keyword_loc().map(|l| l.span()),
            node.statements(),
        );
    }

    fn check_unless(ctx: &mut Context<'_>, node: &UnlessNode<'_>) {
        Self::check(
            ctx,
            "unless",
            node.location().span(),
            node.then_keyword_loc().map(|l| l.span()),
            node.statements(),
        );
    }
}

impl Rule for MultilineIfThen {
    const META: RuleMeta = RuleMeta {
        name: "Style/MultilineIfThen",
        department: Department::Style,
        summary: "Checks for uses of the `then` keyword in multi-line if statements.",
        explanation: "\
Checks for uses of the `then` keyword in multi-line `if` statements.
In multi-line `if` statements, `then` is redundant because the newline
already separates the condition from the body.

```ruby
# bad
# This is considered bad practice.
if cond then
end

# good
# If statements can contain `then` on the same line.
if cond then a
elsif cond then b
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::IfNode { .. } => Self::check_if(ctx, &node.as_if_node().expect("kind matched")),
            Node::UnlessNode { .. } => {
                Self::check_unless(ctx, &node.as_unless_node().expect("kind matched"));
            }
            _ => {}
        }
    }
}
