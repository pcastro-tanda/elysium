//! `Rails/I18nLazyLookup`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/i18n_lazy_lookup.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks for places where I18n "lazy" lookup can be used.
#[derive(Debug, Clone)]
pub struct I18nLazyLookup;

impl Rule for I18nLazyLookup {
    const META: RuleMeta = RuleMeta {
        name: "Rails/I18nLazyLookup",
        department: Department::Rails,
        summary: "Checks for places where I18n \"lazy\" lookup can be used.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let _ = (node, ctx, NodeKind::CallNode);
    }
}
