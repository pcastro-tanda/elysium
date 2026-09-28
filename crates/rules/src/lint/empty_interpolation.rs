//! `Lint/EmptyInterpolation`, ported from RuboCop's
//! `lib/rubocop/cop/lint/empty_interpolation.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Empty interpolation detected.";

/// Checks for empty string interpolation.
#[derive(Debug, Clone)]
pub struct EmptyInterpolation;

impl Rule for EmptyInterpolation {
    const META: RuleMeta = RuleMeta {
        name: "Lint/EmptyInterpolation",
        department: Department::Lint,
        summary: "Checks for empty interpolation.",
        explanation: "\
```ruby
# bad
\"result is #{}\"

# good
\"result is #{some_result}\"
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::EmbeddedStatementsNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(embedded) = node.as_embedded_statements_node() else { return };
        if in_percent_literal_array(ctx) {
            return;
        }

        let is_empty = match embedded.statements() {
            None => true,
            Some(stmts) => stmts.body().iter().all(|child| is_removable(&child)),
        };
        if !is_empty {
            return;
        }

        let span = node.span();
        let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}

/// RuboCop's `in_percent_literal_array?`: the nearest `array` ancestor (if
/// any) is a `%w`/`%i`/`%W`/`%I` percent literal, whose span starts with
/// `%` rather than `[`.
fn in_percent_literal_array(ctx: &Context<'_>) -> bool {
    let Some(array) = ctx.ancestors().iter().rev().find(|info| info.kind == NodeKind::ArrayNode)
    else {
        return false;
    };
    ctx.text(array.span).first() == Some(&b'%')
}

/// RuboCop's `node_children.delete_if { |e| e.nil_type? || (e.basic_literal?
/// && e.str_content&.empty?) }` predicate, i.e. whether `child` contributes
/// nothing to the interpolation's runtime value.
fn is_removable(child: &Node<'_>) -> bool {
    if child.as_nil_node().is_some() {
        return true;
    }
    str_content(child).is_some_and(|content| content.is_empty())
}

/// RuboCop-AST's `str_content`: the literal string content of a `str`/`dstr`/
/// `sym`/`dsym` node, joining `dstr`/`dsym` parts recursively; `None` for any
/// other node type, or if a part isn't itself statically known (e.g. a
/// nested interpolation).
fn str_content(node: &Node<'_>) -> Option<Vec<u8>> {
    if let Some(n) = node.as_string_node() {
        return Some(n.unescaped().to_vec());
    }
    if let Some(n) = node.as_symbol_node() {
        return Some(n.unescaped().to_vec());
    }
    if let Some(n) = node.as_interpolated_string_node() {
        let mut buf = Vec::new();
        for part in &n.parts() {
            buf.extend(str_content(&part)?);
        }
        return Some(buf);
    }
    if let Some(n) = node.as_interpolated_symbol_node() {
        let mut buf = Vec::new();
        for part in &n.parts() {
            buf.extend(str_content(&part)?);
        }
        return Some(buf);
    }
    None
}
