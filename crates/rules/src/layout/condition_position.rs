//! `Layout/ConditionPosition`, ported from RuboCop's
//! `lib/rubocop/cop/layout/condition_position.rb`.
//!
//! Whitequark's single `:if` node type covers both `if` and `unless`
//! (and `elsif`, reached the same way through the same node type);
//! upstream's single `on_if` therefore fires for genuine `unless`
//! statements too. Prism keeps `IfNode` (covering both a real `if` and,
//! through [`ruby_ast::node::IfNode::subsequent`], each nested `elsif`
//! link) and `UnlessNode` separate, so this port subscribes to both.
//!
//! `begin ... end while/until` (a post-condition loop) is a distinct
//! `:while_post`/`:until_post` node type upstream, which this cop never
//! subscribes to; Prism instead sets `WhileNode`/`UntilNode::is_begin_modifier`
//! on the very same node kind used for the ordinary form, so that flag is
//! checked here to skip it the same way.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// One conditional's pieces this cop needs, gathered from whichever of
/// `IfNode`/`UnlessNode`/`WhileNode`/`UntilNode` fired.
struct Conditional {
    /// The `if`/`elsif`/`unless`/`while`/`until` keyword.
    keyword: Span,
    /// The condition expression.
    predicate: Span,
    /// RuboCop's `ModifierNode#modifier_form?` (overridden, for `IfNode`,
    /// to also require the keyword read `if` rather than `elsif`).
    is_modifier: bool,
    /// The body's own span, when it has one -- RuboCop's `node.body`.
    body: Option<Span>,
}

/// Pulls a [`Conditional`] out of `node`, or `None` when `node` is a
/// ternary (no `if_keyword_loc`) or a post-condition `begin...end while`/
/// `until` loop (upstream never visits either).
fn conditional(node: &Node<'_>) -> Option<Conditional> {
    match node.kind() {
        NodeKind::IfNode => {
            let n = node.as_if_node()?;
            let kw = n.if_keyword_loc()?;
            let is_modifier = kw.as_slice() == b"if" && n.end_keyword_loc().is_none();
            Some(Conditional {
                keyword: kw.span(),
                predicate: n.predicate().span(),
                is_modifier,
                body: n.statements().map(|s| s.as_node().span()),
            })
        }
        NodeKind::UnlessNode => {
            let n = node.as_unless_node()?;
            Some(Conditional {
                keyword: n.keyword_loc().span(),
                predicate: n.predicate().span(),
                is_modifier: n.end_keyword_loc().is_none(),
                body: n.statements().map(|s| s.as_node().span()),
            })
        }
        NodeKind::WhileNode => {
            let n = node.as_while_node()?;
            if n.is_begin_modifier() {
                return None;
            }
            Some(Conditional {
                keyword: n.keyword_loc().span(),
                predicate: n.predicate().span(),
                is_modifier: n.closing_loc().is_none(),
                body: n.statements().map(|s| s.as_node().span()),
            })
        }
        NodeKind::UntilNode => {
            let n = node.as_until_node()?;
            if n.is_begin_modifier() {
                return None;
            }
            Some(Conditional {
                keyword: n.keyword_loc().span(),
                predicate: n.predicate().span(),
                is_modifier: n.closing_loc().is_none(),
                body: n.statements().map(|s| s.as_node().span()),
            })
        }
        _ => None,
    }
}

/// Checks for condition placed in a confusing position relative to the keyword.
#[derive(Debug, Clone)]
pub struct ConditionPosition;

impl Rule for ConditionPosition {
    const META: RuleMeta = RuleMeta {
        name: "Layout/ConditionPosition",
        department: Department::Layout,
        summary: "Checks for condition placed in a confusing position relative to the keyword.",
        explanation: "\
A multi-line `if`/`unless`/`while`/`until` reads oddly when its condition
sits on its own line below the keyword:

```ruby
# bad
if
  some_condition
  do_something
end

# good
if some_condition
  do_something
end
```

The fix moves the condition up onto the keyword's own line.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode, NodeKind::WhileNode, NodeKind::UntilNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(cond) = conditional(node) else { return };
        if cond.is_modifier {
            return;
        }
        if ctx.line_col(cond.keyword.start).line == ctx.line_col(cond.predicate.start).line {
            return;
        }

        let keyword_text = String::from_utf8_lossy(ctx.text(cond.keyword));
        let message = format!("Place the condition on the same line as `{keyword_text}`.");

        // RuboCop's `removal_range`: when a body statement shares the
        // condition's last line (separated by `;`), only the condition and
        // its trailing separator are removed, preserving the body
        // statement; otherwise the whole condition line goes, newline
        // included.
        let removal = match cond.body {
            Some(body) if ctx.line_col(body.start).line == ctx.last_line(cond.predicate) => {
                Span::new(cond.predicate.start, body.start)
            }
            _ => ctx.whole_lines(cond.predicate),
        };

        let mut insertion =
            Vec::with_capacity(1 + (cond.predicate.end - cond.predicate.start) as usize);
        insertion.push(b' ');
        insertion.extend_from_slice(ctx.text(cond.predicate));

        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::insert(cond.keyword.end, insertion), Edit::delete(removal)],
        };
        ctx.report_with_fix(&Self::META, cond.predicate, message, fix);
    }
}
