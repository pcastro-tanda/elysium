//! `Minitest/UnspecifiedException`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/unspecified_exception.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// This cop checks for a specified error in `assert_raises`.
#[derive(Debug, Clone)]
pub struct UnspecifiedException;

impl Rule for UnspecifiedException {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/UnspecifiedException",
        department: Department::Minitest,
        summary: "This cop checks for a specified error in `assert_raises`.",
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
