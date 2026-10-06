//! `Rails/TransactionExitStatement`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/transaction_exit_statement.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Avoid the usage of `return`, `break` and `throw` in transaction blocks.
#[derive(Debug, Clone)]
pub struct TransactionExitStatement;

impl Rule for TransactionExitStatement {
    const META: RuleMeta = RuleMeta {
        name: "Rails/TransactionExitStatement",
        department: Department::Rails,
        summary: "Avoid the usage of `return`, `break` and `throw` in transaction blocks.",
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
