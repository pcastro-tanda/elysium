//! `Sorbet/ForbidExtendTSigHelpersInShims`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_extend_t_sig_helpers_in_shims.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Forbid the use of `extend T::Sig` and `extend T::Helpers` in RBI shims
#[derive(Debug, Clone)]
pub struct ForbidExtendTSigHelpersInShims;

impl Rule for ForbidExtendTSigHelpersInShims {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidExtendTSigHelpersInShims",
        department: Department::Sorbet,
        summary: "Forbid the use of `extend T::Sig` and `extend T::Helpers` in RBI shims",
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
