//! `Layout/SpaceBeforeBrackets`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_before_brackets.rb`.
//!
//! RuboCop's `node.loc.dot` is set whenever the call used an explicit `.`
//! (or `&.`) before the method name, which for a `[]`/`[]=` call only
//! happens with desugared call syntax (`collection.[](key)`). Prism gives
//! that form an `opening_loc` of `(`, not `[` (see
//! `space_inside_reference_brackets.rs`'s module docs for the same trap),
//! so filtering on a literal `[` opening reproduces the `loc.dot` guard
//! without needing `call_operator_loc` at all. `on_send` is never aliased
//! to `on_csend` upstream, so a safe-navigation `[]` call is silently
//! skipped too.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Remove the space before the opening brackets.";

/// Checks for space between the name of a receiver and a left bracket.
#[derive(Debug, Clone)]
pub struct SpaceBeforeBrackets;

impl Rule for SpaceBeforeBrackets {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceBeforeBrackets",
        department: Department::Layout,
        summary: "Checks for receiver with a space before the opening brackets.",
        explanation: "\
```ruby
# bad
collection [index_or_key]

# good
collection[index_or_key]
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        if call.is_safe_navigation() || !matches!(call.name().as_slice(), b"[]" | b"[]=") {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(opening) = call.opening_loc() else { return };
        if opening.as_slice() != b"[" {
            return;
        }
        let receiver_end = receiver.span().end;
        let selector_begin = opening.span().start;
        if receiver_end >= selector_begin {
            return;
        }
        let range = Span::new(receiver_end, selector_begin);
        ctx.report_with_fix(
            &Self::META,
            range,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(range)] },
        );
    }
}
