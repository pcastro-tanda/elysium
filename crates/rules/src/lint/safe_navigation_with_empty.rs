//! `Lint/SafeNavigationWithEmpty`, ported from RuboCop's
//! `lib/rubocop/cop/lint/safe_navigation_with_empty.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Avoid calling `empty?` with the safe navigation operator in conditionals.";

/// Avoid `foo&.empty?` in conditionals.
#[derive(Debug, Clone)]
pub struct SafeNavigationWithEmpty;

impl Rule for SafeNavigationWithEmpty {
    const META: RuleMeta = RuleMeta {
        name: "Lint/SafeNavigationWithEmpty",
        department: Department::Lint,
        summary: "Avoid `foo&.empty?` in conditionals.",
        explanation: "Checks to make sure safe navigation isn't used with `empty?` in \
                      a conditional.\n\nWhile the safe navigation operator is generally a good \
                      idea, when checking `foo&.empty?` in a conditional, `foo` being `nil` \
                      will actually do the opposite of what the author intends.",
        enabled_by_default: true,
        severity: Severity::Warning,
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
        let predicate = match node.kind() {
            NodeKind::IfNode => node.as_if_node().map(|n| n.predicate()),
            NodeKind::UnlessNode => node.as_unless_node().map(|n| n.predicate()),
            _ => None,
        };
        let Some(predicate) = predicate else { return };
        let Some(call) = predicate.as_call_node() else { return };
        if !call.is_safe_navigation() || call.name().as_slice() != b"empty?" {
            return;
        }
        // RuboCop 1.91: `(csend !csend :empty?)` -- any receiver except
        // another safe-navigation call.
        let Some(receiver) = call.receiver() else { return };
        if receiver.as_call_node().is_some_and(|r| r.is_safe_navigation()) {
            return;
        }

        let span = predicate.span();
        let receiver_text = String::from_utf8_lossy(ctx.text(receiver.span())).into_owned();
        let replacement = format!("{receiver_text} && {receiver_text}.empty?");

        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement.into_bytes())],
            },
        );
    }
}
