//! `Rails/RedundantReceiverInWithOptions`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/redundant_receiver_in_with_options.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks for redundant receiver in `with_options`.
#[derive(Debug, Clone)]
pub struct RedundantReceiverInWithOptions;

impl Rule for RedundantReceiverInWithOptions {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RedundantReceiverInWithOptions",
        department: Department::Rails,
        summary: "Checks for redundant receiver in `with_options`.",
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
