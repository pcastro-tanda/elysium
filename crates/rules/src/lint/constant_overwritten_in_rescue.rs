//! `Lint/ConstantOverwrittenInRescue`, ported from RuboCop's
//! `lib/rubocop/cop/lint/constant_overwritten_in_rescue.rb`.
//!
//! Upstream's node-pattern matcher, `(resbody nil? $(casgn _ _) nil?)`,
//! requires: no exception class list (`rescue => Foo`, not `rescue Foo =>
//! Foo`), the `=>` target parsing as a constant assignment (`casgn`), and an
//! empty rescue body. In Prism, the `=>` target of a bare `rescue => Foo` is
//! a `ConstantTargetNode` (or `ConstantPathTargetNode` for `rescue =>
//! Foo::Bar`/`rescue => foo.class::X`) rather than a write node, since it is
//! never actually assigned to (that is the whole bug this cop reports); the
//! other two conditions (`exceptions.is_empty()`, no `statements`) carry
//! over directly from `RescueNode`'s own fields.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks for overwriting an exception with an exception result by using `rescue =>`.
#[derive(Debug, Clone)]
pub struct ConstantOverwrittenInRescue;

impl Rule for ConstantOverwrittenInRescue {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ConstantOverwrittenInRescue",
        department: Department::Lint,
        summary:
            "Checks for overwriting an exception with an exception result by using `rescue =>`.",
        explanation: "\
Checks for overwriting an exception with an exception result by using ``rescue =>``.

You intended to write as `rescue StandardError`. However, you have written \
`rescue => StandardError`. In that case, the result of `rescue` will \
overwrite `StandardError`.

```ruby
# bad
begin
  something
rescue => StandardError
end

# good
begin
  something
rescue StandardError
end
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::RescueNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(rescue) = node.as_rescue_node() else { return };
        if !rescue.exceptions().is_empty() || rescue.statements().is_some() {
            return;
        }
        let Some(reference) = rescue.reference() else { return };
        if !matches!(
            reference.kind(),
            NodeKind::ConstantTargetNode | NodeKind::ConstantPathTargetNode
        ) {
            return;
        }
        let Some(operator_loc) = rescue.operator_loc() else { return };

        let constant_source = String::from_utf8_lossy(ctx.text(reference.span()));
        let message = format!("`{constant_source}` is overwritten by `rescue =>`.");

        let removal = Span::new(rescue.keyword_loc().span().end, operator_loc.span().end);
        let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(removal)] };
        ctx.report_with_fix(&Self::META, operator_loc.span(), message, fix);
    }
}
