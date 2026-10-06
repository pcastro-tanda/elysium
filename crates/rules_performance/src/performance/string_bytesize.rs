//! `Performance/StringBytesize`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/string_bytesize.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `String#bytesize` instead of calculating the size of the bytes array.
#[derive(Debug, Clone)]
pub struct StringBytesize;

impl Rule for StringBytesize {
    const META: RuleMeta = RuleMeta {
        name: "Performance/StringBytesize",
        department: Department::Performance,
        summary: "Use `String#bytesize` instead of calculating the size of the bytes array.",
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
