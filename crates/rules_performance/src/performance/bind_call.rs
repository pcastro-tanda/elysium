//! `Performance/BindCall`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/bind_call.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeKind};

/// Use `bind_call(obj, args, ...)` instead of `bind(obj).call(args, ...)`.
#[derive(Debug, Clone)]
pub struct BindCall;

impl Rule for BindCall {
    const META: RuleMeta = RuleMeta {
        name: "Performance/BindCall",
        department: Department::Performance,
        summary: "Use `bind_call(obj, args, ...)` instead of `bind(obj).call(args, ...)`.",
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
