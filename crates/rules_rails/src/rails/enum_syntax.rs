//! `Rails/EnumSyntax`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/enum_syntax.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use positional arguments over keyword arguments when defining enums.
#[derive(Debug, Clone)]
pub struct EnumSyntax;

impl Rule for EnumSyntax {
    const META: RuleMeta = RuleMeta {
        name: "Rails/EnumSyntax",
        department: Department::Rails,
        summary: "Use positional arguments over keyword arguments when defining enums.",
        explanation: "",
        enabled_by_default: false,
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
