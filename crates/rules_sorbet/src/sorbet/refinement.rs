//! `Sorbet/Refinement`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/refinement.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Do not use Ruby Refinements library as it is not supported by Sorbet.";

/// Checks for the use of Ruby Refinements library.
#[derive(Debug, Clone)]
pub struct Refinement;

impl Rule for Refinement {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/Refinement",
        department: Department::Sorbet,
        summary: "Checks for the use of Ruby Refinements library. Refinements add complexity and incur a performance penalty that can be significant for large code bases. They are also not supported by Sorbet.",
        explanation: "Checks for the use of Ruby Refinements library. Refinements add\ncomplexity and incur a performance penalty that can be significant\nfor large code bases. Good examples are cases of unrelated\nmethods that happen to have the same name as these module methods.\n\n```ruby\n# bad\nmodule Foo\n  refine(Date) do\n  end\nend\n\n# bad\nmodule Foo\n  using(Date) do\n  end\nend\n\n# good\nmodule Foo\n  bar.refine(Date)\nend\n\n# good\nmodule Foo\n  bar.using(Date)\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return;
        }
        let name = call.name();
        let is_refine = name.as_slice() == b"refine";
        if !is_refine && name.as_slice() != b"using" {
            return;
        }
        if call.receiver().is_some() {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let Some(first) = arguments.arguments().iter().next() else { return };
        if !matches!(first.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode) {
            return;
        }
        if is_refine {
            if call.block().is_none_or(|block| block.as_block_node().is_none()) {
                return;
            }
            // `node.parent.parent.module_type?`: the block is the module's
            // only body statement (whitequark adds a `begin` otherwise).
            let ancestors = ctx.ancestors();
            let [.., grandparent, parent] = ancestors else { return };
            if parent.kind != NodeKind::StatementsNode
                || parent.span != node.span()
                || grandparent.kind != NodeKind::ModuleNode
            {
                return;
            }
        }
        ctx.report(&Self::META, call_span_excluding_block(&call), MSG);
    }
}
