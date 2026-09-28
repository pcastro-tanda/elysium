//! `Lint/TrailingCommaInAttributeDeclaration`, ported from RuboCop's
//! `lib/rubocop/cop/lint/trailing_comma_in_attribute_declaration.rb`.
//!
//! Upstream detects this by noticing that a trailing comma after an
//! `attr_reader`/`attr_writer`/`attr_accessor`/`attr` call's last real
//! argument makes the parser swallow the following `def` into the call's
//! argument list (`node.last_argument.def_type?`). Prism parses the same
//! source the same way -- the stray `def` shows up as the last element of
//! the `CallNode`'s `ArgumentsNode` -- so the same shape check applies
//! verbatim.
//!
//! The trailing comma's location is derived the same way upstream's
//! `trailing_comma_range` does: starting right after the second-to-last
//! argument (the last "real", non-`def` one), skip spaces/tabs, and the
//! next byte is the comma.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MSG: &str = "Avoid leaving a trailing comma in attribute declarations.";

/// Checks for trailing commas in attribute declarations.
#[derive(Debug, Clone)]
pub struct TrailingCommaInAttributeDeclaration;

impl Rule for TrailingCommaInAttributeDeclaration {
    const META: RuleMeta = RuleMeta {
        name: "Lint/TrailingCommaInAttributeDeclaration",
        department: Department::Lint,
        summary: "Checks for trailing commas in attribute declarations.",
        explanation: "\
Checks for trailing commas in attribute declarations, such as
`#attr_reader`. Leaving a trailing comma will nullify the next method
definition by overriding it with a getter method.

```ruby
# bad
class Foo
  attr_reader :foo,

  def bar
    puts \"Unreachable.\"
  end
end

# good
class Foo
  attr_reader :foo

  def bar
    puts \"No problem!\"
  end
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
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
        let Some(call) = node.as_call_node() else { return };
        if !is_attribute_accessor(&call) {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let args = args.arguments();
        if args.len() < 2 {
            return;
        }
        let Some(last) = args.iter().last() else { return };
        if last.as_def_node().is_none() {
            return;
        }
        // `arguments[-2]`: the last "real" argument, immediately before the
        // `def` that got swallowed in by the trailing comma.
        let second_to_last = args.iter().nth(args.len() - 2).expect("checked len >= 2");

        let span = trailing_comma_span(ctx, second_to_last.location().span());
        let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}

/// RuboCop's `SendNode#attribute_accessor?`: a receiver-less call to
/// `attr_reader`/`attr_writer`/`attr_accessor`/`attr`.
fn is_attribute_accessor(call: &CallNode<'_>) -> bool {
    call.receiver().is_none()
        && matches!(
            call.name().as_slice(),
            b"attr_reader" | b"attr_writer" | b"attr_accessor" | b"attr"
        )
}

/// RuboCop's `trailing_comma_range`: from the end of `prev_arg_span`, skip
/// spaces/tabs (matching `range_with_surrounding_space(side: :right)`'s
/// `[ \t]` skip), then take the single byte there -- the comma.
fn trailing_comma_span(ctx: &Context<'_>, prev_arg_span: Span) -> Span {
    let source = ctx.source().bytes();
    let mut pos = prev_arg_span.end as usize;
    while matches!(source.get(pos), Some(b' ' | b'\t')) {
        pos += 1;
    }
    let start = u32::try_from(pos).expect("offset exceeds u32");
    Span::new(start, start + 1)
}
