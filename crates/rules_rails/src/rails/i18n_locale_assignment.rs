//! `Rails/I18nLocaleAssignment`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/i18n_locale_assignment.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `I18n.with_locale` with block instead of `I18n.locale=`.";

/// Prefer the usage of `I18n.with_locale` instead of manually updating `I18n.locale` value.
#[derive(Debug, Clone)]
pub struct I18nLocaleAssignment;

impl Rule for I18nLocaleAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Rails/I18nLocaleAssignment",
        department: Department::Rails,
        summary: "Prefer the usage of `I18n.with_locale` instead of manually updating `I18n.locale` value.",
        explanation: "Checks for the use of `I18n.locale=` method.\n\nThe `locale` attribute persists for the rest of the Ruby runtime, potentially causing unexpected behavior at a later time. Using `I18n.with_locale` ensures the code passed in the block is the only place `I18n.locale` is affected. It eliminates the possibility of a `locale` sticking around longer than intended.\n\n```ruby\n# bad\nI18n.locale = :fr\n\n# good\nI18n.with_locale(:fr) do\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode, NodeKind::CallTargetNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        // `(send (const {nil? cbase} :I18n) :locale= ...)`; a multiple-assignment
        // target is such a `send` too.
        let (receiver, name) = if let Some(call) = node.as_call_node() {
            (call.receiver(), call.name())
        } else if let Some(target) = node.as_call_target_node() {
            (Some(target.receiver()), target.name())
        } else {
            return;
        };
        if name.as_slice() != b"locale=" {
            return;
        }
        let Some(receiver) = receiver else { return };
        if is_bare_or_toplevel_const(&receiver) && const_name(&receiver).as_deref() == Some("I18n")
        {
            ctx.report(&Self::META, node.span(), MSG);
        }
    }
}
