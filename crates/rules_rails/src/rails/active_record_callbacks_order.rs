//! `Rails/ActiveRecordCallbacksOrder`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/active_record_callbacks_order.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Order callback declarations in the order in which they will be executed.
#[derive(Debug, Clone)]
pub struct ActiveRecordCallbacksOrder;

impl Rule for ActiveRecordCallbacksOrder {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ActiveRecordCallbacksOrder",
        department: Department::Rails,
        summary: "Order callback declarations in the order in which they will be executed.",
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
