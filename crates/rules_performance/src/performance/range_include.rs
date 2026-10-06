//! `Performance/RangeInclude`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/range_include.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// Use `Range#cover?` instead of `Range#include?` (or `Range#member?`).
#[derive(Debug, Clone)]
pub struct RangeInclude;

impl Rule for RangeInclude {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RangeInclude",
        department: Department::Performance,
        summary: "Use `Range#cover?` instead of `Range#include?` (or `Range#member?`).",
        explanation: "Identifies uses of `Range#include?` and `Range#member?`, which iterates \
                      over each item in a `Range` to see if a specified item is there. In \
                      contrast, `Range#cover?` simply compares the target item with the \
                      beginning and end points of the `Range`.\n\nThis cop is unsafe because \
                      `Range#include?` (or `Range#member?`) and `Range#cover?` are not \
                      equivalent behavior.\n\n```ruby\n# bad\n('a'..'z').include?('b') # => \
                      true\n('a'..'z').member?('b')  # => true\n\n# good\n('a'..'z').cover?('b') \
                      # => true\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let bad_method = match name.as_slice() {
            b"include?" => "include?",
            b"member?" => "member?",
            _ => return,
        };
        let Some(receiver) = call.receiver() else { return };
        if !is_range_or_parenthesized_range(&receiver) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let span = selector.span();
        ctx.report_with_fix(
            &Self::META,
            span,
            format!("Use `Range#cover?` instead of `Range#{bad_method}`."),
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, b"cover?".to_vec())],
            },
        );
    }
}

/// `{range (begin range)}`.
fn is_range_or_parenthesized_range(node: &Node<'_>) -> bool {
    if node.as_range_node().is_some() {
        return true;
    }
    let Some(parens) = node.as_parentheses_node() else { return false };
    let Some(body) = parens.body().and_then(|body| body.as_statements_node()) else {
        return false;
    };
    let mut statements = body.body().iter();
    matches!((statements.next(), statements.next()), (Some(only), None) if only.as_range_node().is_some())
}
