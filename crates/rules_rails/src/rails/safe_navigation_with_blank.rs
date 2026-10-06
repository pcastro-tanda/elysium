//! `Rails/SafeNavigationWithBlank`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/safe_navigation_with_blank.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "Avoid calling `blank?` with the safe navigation operator in conditionals.";

/// Avoid `foo&.blank?` in conditionals.
#[derive(Debug, Clone)]
pub struct SafeNavigationWithBlank;

impl Rule for SafeNavigationWithBlank {
    const META: RuleMeta = RuleMeta {
        name: "Rails/SafeNavigationWithBlank",
        department: Department::Rails,
        summary: "Avoid `foo&.blank?` in conditionals.",
        explanation: "Checks to make sure safe navigation isn't used with `blank?` in a \
                      conditional.\n\nWhile the safe navigation operator is generally a good \
                      idea, when checking `foo&.blank?` in a conditional, `foo` being `nil` \
                      will actually do the opposite of what the author intends: `foo&.blank?` \
                      is `nil` whereas `foo.blank?` is `true`.\n\n```ruby\n# bad\n\
                      do_something if foo&.blank?\ndo_something unless foo&.blank?\n\n\
                      # good\ndo_something if foo.blank?\ndo_something unless foo.blank?\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    /// `(if $(csend ... :blank?) ...)`; Prism has `UnlessNode` where
    /// whitequark has `if` with swapped branches.
    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let predicate = match node.kind() {
            NodeKind::IfNode => node.as_if_node().map(|n| n.predicate()),
            _ => node.as_unless_node().map(|n| n.predicate()),
        };
        let Some(predicate) = predicate else { return };
        let Some(call) = predicate.as_call_node() else { return };
        if !call.is_safe_navigation() || call.name().as_slice() != b"blank?" {
            return;
        }
        let Some(dot) = call.call_operator_loc() else { return };
        let dot = dot.span();
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(dot, b".".to_vec())],
            },
        );
    }
}
