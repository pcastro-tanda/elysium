//! `Style/IfUnlessModifierOfIfUnless`, ported from RuboCop's
//! `lib/rubocop/cop/style/if_unless_modifier_of_if_unless.rb`. The cop
//! includes the `StatementModifier` mixin but never calls any of its
//! methods, so nothing from it is ported here.
//!
//! Whitequark's parser gives `if`, `unless`, and ternary (`a ? b : c`) all
//! the same `:if` node type, so RuboCop's `node.body.if_type?` matches any
//! of the three. Prism instead gives `if`/ternary a shared [`NodeKind::IfNode`]
//! and `unless` its own [`NodeKind::UnlessNode`]; [`is_conditional`] checks
//! both kinds to reproduce the same umbrella check.

use std::borrow::Cow;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::StatementsNode;
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
fn message(keyword: &str) -> String {
    format!("Avoid modifier `{keyword}` after another conditional.")
}

/// A modifier-form `if`/`unless` node, normalized across both Prism kinds
/// (mirroring rubocop-ast's `IfNode#keyword`/`#modifier_form?`/`#condition`/
/// `#if_branch`, unified over whitequark's single `:if` node type).
struct Modifier<'pr> {
    keyword: &'static str,
    keyword_span: Span,
    condition: Node<'pr>,
    /// The single statement executed when the condition holds
    /// (rubocop-ast's `IfNode#if_branch`, normalized for `unless`).
    body: Node<'pr>,
}

/// The lone statement of a `StatementsNode`, or `None` for zero or several
/// -- matching rubocop-ast's `node_parts`, which only omits the synthetic
/// `begin` wrapper (and so only unifies with a single statement).
fn single_statement(stmts: Option<StatementsNode<'_>>) -> Option<Node<'_>> {
    let stmts = stmts?;
    let mut items = stmts.body().iter();
    let first = items.next()?;
    if items.next().is_some() {
        return None;
    }
    Some(first)
}

/// Builds a [`Modifier`] for `node`, or `None` when it is not a modifier-form
/// `if`/`unless` (RuboCop's `node.modifier_form?`: has a real `if`/`unless`
/// keyword -- excluding `elsif`, and ternaries, which never carry one -- and
/// no `end` keyword).
fn modifier_shape<'pr>(node: &Node<'pr>) -> Option<Modifier<'pr>> {
    match node {
        Node::IfNode { .. } => {
            let n = node.as_if_node().expect("kind matched");
            let kw_loc = n.if_keyword_loc()?;
            if kw_loc.as_slice() == b"elsif" || n.end_keyword_loc().is_some() {
                return None;
            }
            let body = single_statement(n.statements())?;
            Some(Modifier {
                keyword: "if",
                keyword_span: kw_loc.span(),
                condition: n.predicate(),
                body,
            })
        }
        Node::UnlessNode { .. } => {
            let n = node.as_unless_node().expect("kind matched");
            if n.end_keyword_loc().is_some() {
                return None;
            }
            let body = single_statement(n.statements())?;
            Some(Modifier {
                keyword: "unless",
                keyword_span: n.keyword_loc().span(),
                condition: n.predicate(),
                body,
            })
        }
        _ => None,
    }
}

/// RuboCop's `body.if_type?`: whitequark unifies `if`, `unless`, and ternary
/// under one `:if` node type, so any of Prism's two conditional kinds
/// qualifies.
fn is_conditional(node: &Node<'_>) -> bool {
    matches!(node, Node::IfNode { .. } | Node::UnlessNode { .. })
}

/// Builds RuboCop's autocorrection:
/// ```ruby
/// corrector.wrap(node.if_branch, "#{node.keyword} #{node.condition.source}\n", "\nend")
/// corrector.remove(node.if_branch.source_range.end.join(node.condition.source_range.end))
/// ```
fn build_fix(shape: &Modifier<'_>, ctx: &Context<'_>) -> Fix {
    let body_span = shape.body.location().span();
    let condition_span = shape.condition.location().span();
    let condition_text = String::from_utf8_lossy(ctx.text(condition_span));
    let prefix = format!("{} {condition_text}\n", shape.keyword);
    Fix {
        applicability: Applicability::Safe,
        edits: vec![
            Edit::insert(body_span.start, prefix.into_bytes()),
            Edit::insert(body_span.end, b"\nend".to_vec()),
            Edit::delete(Span::new(body_span.end, condition_span.end)),
        ],
    }
}

/// Checks for `if` and `unless` statements used as modifiers of other `if`
/// or `unless` statements.
#[derive(Debug, Clone)]
pub struct IfUnlessModifierOfIfUnless;

impl Rule for IfUnlessModifierOfIfUnless {
    const META: RuleMeta = RuleMeta {
        name: "Style/IfUnlessModifierOfIfUnless",
        department: Department::Style,
        summary: "Avoid modifier if/unless usage on conditionals.",
        explanation: "\
Checks for `if` and `unless` statements used as modifiers of other `if` or
`unless` statements.

```ruby
# bad
tired? ? 'stop' : 'go faster' if running?

# bad
if tired?
  \"please stop\"
else
  \"keep going\"
end if running?

# good
if running?
  tired? ? 'stop' : 'go faster'
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[],
        blind_spots: "\
A modifier `if`/`unless` whose body is a nested modifier `if`/`unless` (e.g.
`a if b if c`) needs two fix-and-reparse rounds to reach RuboCop's final
correction, since the outer node's fix would otherwise collide with the
inner node's insertion at the same offset; the engine's fix loop already
reruns rules to convergence, so this only affects how many rounds it takes,
not the final output.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn leave(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(shape) = modifier_shape(node) else { return };
        if !is_conditional(&shape.body) {
            return;
        }
        let fix = build_fix(&shape, ctx);
        ctx.report_with_fix(
            &Self::META,
            shape.keyword_span,
            Cow::Owned(message(shape.keyword)),
            fix,
        );
    }
}
