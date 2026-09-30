//! `Layout/SpaceAfterMethodName`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_after_method_name.rb`.
//!
//! Upstream's `on_def` (aliased as `on_defs`) reads `node.arguments`'s own
//! `parenthesized_call?` -- whitequark's `args` node carries the enclosing
//! `(`/`)` locations as its own `begin`/`end`, even for an empty parameter
//! list (`def f() end`). Prism instead puts `lparen_loc`/`rparen_loc`
//! directly on the surrounding [`NodeKind::DefNode`] (covering both a plain
//! `def` and a singleton `def self.foo`, so no separate `on_defs` handler is
//! needed here), independent of whether `parameters()` is present at all.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Do not put a space between a method name and the opening parenthesis.";

/// Do not put a space between a method name and the opening parenthesis in a method definition.
#[derive(Debug, Clone)]
pub struct SpaceAfterMethodName;

impl Rule for SpaceAfterMethodName {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceAfterMethodName",
        department: Department::Layout,
        summary: "Do not put a space between a method name and the opening parenthesis in a method definition.",
        explanation: "Checks for space between a method name and a left parenthesis in defs.\n\n\
```ruby\n# bad\ndef func (x) end\ndef method= (y) end\n\n# good\ndef func(x) end\ndef method=(y) end\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let def = node.as_def_node().expect("kind matched");
        let Some(lparen) = def.lparen_loc() else { return };
        let lparen_start = lparen.span().start;
        let Some(before_start) = lparen_start.checked_sub(1) else { return };
        let before = Span::new(before_start, lparen_start);
        if ctx.text(before) != b" " {
            return;
        }
        ctx.report_with_fix(
            &Self::META,
            before,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(before)] },
        );
    }
}
