//! `Style/BeginBlock`, ported from RuboCop's
//! `lib/rubocop/cop/style/begin_block.rb`.
//!
//! Ruby's `BEGIN { ... }` construct parses as its own dedicated Prism node
//! kind, [`NodeKind::PreExecutionNode`] (mirroring whitequark's `preexe`),
//! rather than as an ordinary block-call -- so no method-name filtering is
//! needed to tell it apart from anything else.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Avoid the use of `BEGIN` blocks.";

/// Checks for `BEGIN` blocks.
#[derive(Debug, Clone, Default)]
pub struct BeginBlock;

impl Rule for BeginBlock {
    const META: RuleMeta = RuleMeta {
        name: "Style/BeginBlock",
        department: Department::Style,
        summary: "Checks for BEGIN blocks.",
        explanation: "\
Checks for BEGIN blocks.

```ruby
# bad
BEGIN { test }
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::PreExecutionNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(preexe) = node.as_pre_execution_node() {
            ctx.report(&Self::META, preexe.keyword_loc().span(), MSG);
        }
    }
}
