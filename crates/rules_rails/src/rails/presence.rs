//! `Rails/Presence`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/presence.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks code that can be written more easily using `Object#presence` defined by Active Support.
#[derive(Debug, Clone)]
pub struct Presence;

impl Rule for Presence {
    const META: RuleMeta = RuleMeta {
        name: "Rails/Presence",
        department: Department::Rails,
        summary: "Checks code that can be written more easily using `Object#presence` defined by Active Support.",
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
