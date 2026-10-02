//! `Rails/ArelStar`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/arel_star.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `Arel.star` instead of `\"*\"` for expanded column lists.";

/// Prevents usage of `"*"` on an `Arel::Table` column reference.
#[derive(Debug, Clone)]
pub struct ArelStar;

impl Rule for ArelStar {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ArelStar",
        department: Department::Rails,
        summary: "Enforces `Arel.star` instead of `\"*\"` for expanded columns.",
        explanation: "Prevents usage of `\"*\"` on an Arel::Table column reference.\n\nUsing \
                      `arel_table[\"\\*\"]` causes the outputted string to be a literal quoted \
                      asterisk (e.g. `my_model`.`*`). This causes the database to look for a \
                      column named `\\*` (or `\"*\"`) as opposed to expanding the column list \
                      as one would likely expect.\n\nThis cop's autocorrection is unsafe \
                      because it turns a quoted `\\*` into an SQL `*`, unquoted. `\\*` is a \
                      valid column name in certain databases supported by Rails, and even \
                      though it is usually a mistake, it might denote legitimate access to a \
                      column named `*`.\n\n```ruby\n# bad\nMyTable.arel_table[\"*\"]\n\n# \
                      good\nMyTable.arel_table[Arel.star]\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    /// `(send {const (send _ :arel_table)} :[] $(str "*"))`.
    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"[]" || call.is_safe_navigation() || call.block().is_some() {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let receiver_matches = match receiver.kind() {
            NodeKind::ConstantReadNode | NodeKind::ConstantPathNode => true,
            NodeKind::CallNode => receiver.as_call_node().is_some_and(|inner| {
                inner.name().as_slice() == b"arel_table" && !inner.is_safe_navigation()
            }),
            _ => false,
        };
        if !receiver_matches {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut arguments = arguments.arguments().iter();
        let (Some(star), None) = (arguments.next(), arguments.next()) else { return };
        let Some(string) = star.as_string_node() else { return };
        if string.unescaped() != b"*" {
            return;
        }
        let span = star.span();
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, b"Arel.star".to_vec())],
            },
        );
    }
}
