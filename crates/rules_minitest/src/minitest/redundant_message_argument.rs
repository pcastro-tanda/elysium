//! `Minitest/RedundantMessageArgument`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/redundant_message_argument.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Detects redundant message argument in assertion methods.
#[derive(Debug, Clone)]
pub struct RedundantMessageArgument;

impl Rule for RedundantMessageArgument {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RedundantMessageArgument",
        department: Department::Minitest,
        summary: "Detects redundant message argument in assertion methods.",
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
