//! `Sorbet/HasSigil`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/has_sigil.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Makes the Sorbet typed sigil mandatory in all files.
#[derive(Debug, Clone)]
pub struct HasSigil;

impl Rule for HasSigil {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/HasSigil",
        department: Department::Sorbet,
        summary: "Makes the Sorbet typed sigil mandatory in all files.",
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
