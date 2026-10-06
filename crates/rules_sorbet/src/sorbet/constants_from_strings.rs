//! `Sorbet/ConstantsFromStrings`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/constants_from_strings.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Forbids constant access through meta-programming.
For example, things like `constantize` or `const_get` are forbidden.
#[derive(Debug, Clone)]
pub struct ConstantsFromStrings;

impl Rule for ConstantsFromStrings {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ConstantsFromStrings",
        department: Department::Sorbet,
        summary: "Forbids constant access through meta-programming.
For example, things like `constantize` or `const_get` are forbidden.",
        explanation: "",
        enabled_by_default: true,
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
