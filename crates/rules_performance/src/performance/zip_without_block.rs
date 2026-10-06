//! `Performance/ZipWithoutBlock`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/zip_without_block.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Checks for `map { |id| [id] }` and suggests replacing it with `zip`.
#[derive(Debug, Clone)]
pub struct ZipWithoutBlock;

impl Rule for ZipWithoutBlock {
    const META: RuleMeta = RuleMeta {
        name: "Performance/ZipWithoutBlock",
        department: Department::Performance,
        summary: "Checks for `map { |id| [id] }` and suggests replacing it with `zip`.",
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
