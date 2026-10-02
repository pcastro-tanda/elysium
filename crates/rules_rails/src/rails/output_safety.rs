//! `Rails/OutputSafety`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/output_safety.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// The use of `html_safe` or `raw` may be a security risk.
#[derive(Debug, Clone)]
pub struct OutputSafety;

impl Rule for OutputSafety {
    const META: RuleMeta = RuleMeta {
        name: "Rails/OutputSafety",
        department: Department::Rails,
        summary: "The use of `html_safe` or `raw` may be a security risk.",
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
