//! `Performance/BigDecimalWithNumericArgument`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/big_decimal_with_numeric_argument.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Convert numeric literal to string and pass it to `BigDecimal`.
#[derive(Debug, Clone)]
pub struct BigDecimalWithNumericArgument;

impl Rule for BigDecimalWithNumericArgument {
    const META: RuleMeta = RuleMeta {
        name: "Performance/BigDecimalWithNumericArgument",
        department: Department::Performance,
        summary: "Convert numeric literal to string and pass it to `BigDecimal`.",
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
