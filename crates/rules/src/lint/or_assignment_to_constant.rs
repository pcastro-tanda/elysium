//! `Lint/OrAssignmentToConstant`, ported from RuboCop's
//! `lib/rubocop/cop/lint/or_assignment_to_constant.rb`.
//!
//! Whitequark's `on_or_asgn` fires once for either a simple or a qualified
//! constant target (`node.lhs&.casgn_type?`); Prism splits these into
//! `ConstantOrWriteNode`/`ConstantPathOrWriteNode`, both subscribed here.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// Upstream's `MSG`.
const MSG: &str = "Avoid using or-assignment with constants.";

/// Checks unintended or-assignment to constant.
#[derive(Debug, Clone)]
pub struct OrAssignmentToConstant;

impl Rule for OrAssignmentToConstant {
    const META: RuleMeta = RuleMeta {
        name: "Lint/OrAssignmentToConstant",
        department: Department::Lint,
        summary: "Checks unintended or-assignment to constant.",
        explanation: "\
Constants should always be assigned in the same location. And its value
should always be the same. If constants are assigned in multiple
locations, the result may vary depending on the order of `require`.

@safety
  This cop is unsafe because code that is already conditionally
  assigning a constant may have its behavior changed by autocorrection.

```ruby
# bad
CONST ||= 1

# good
CONST = 1
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ConstantOrWriteNode, NodeKind::ConstantPathOrWriteNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let operator_span = match node.kind() {
            NodeKind::ConstantOrWriteNode => {
                node.as_constant_or_write_node().map(|n| n.operator_loc().span())
            }
            NodeKind::ConstantPathOrWriteNode => {
                node.as_constant_path_or_write_node().map(|n| n.operator_loc().span())
            }
            _ => None,
        };
        let Some(operator_span) = operator_span else { return };

        let in_def = ctx.ancestors().iter().any(|a| a.kind == NodeKind::DefNode);
        if in_def {
            ctx.report(&Self::META, operator_span, MSG);
        } else {
            let fix = Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(operator_span, b"=".to_vec())],
            };
            ctx.report_with_fix(&Self::META, operator_span, MSG, fix);
        }
    }
}
