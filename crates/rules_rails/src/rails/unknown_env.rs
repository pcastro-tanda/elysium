//! `Rails/UnknownEnv`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/unknown_env.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use correct environment name.
#[derive(Debug, Clone)]
pub struct UnknownEnv;

impl Rule for UnknownEnv {
    const META: RuleMeta = RuleMeta {
        name: "Rails/UnknownEnv",
        department: Department::Rails,
        summary: "Use correct environment name.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Warning,
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
