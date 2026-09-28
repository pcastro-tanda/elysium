//! `Lint/RedundantWithObject`, ported from RuboCop's
//! `lib/rubocop/cop/lint/redundant_with_object.rb`.
//!
//! RuboCop's `on_block`/`on_numblock`/`on_itblock` all fire on whitequark's
//! unified block-with-its-call node; Prism instead attaches a literal block
//! to the [`ruby_ast::node::CallNode`] it belongs to via `call.block()`, so
//! this rule dispatches on [`NodeKind::CallNode`] directly and inspects the
//! attached block for the `(args (arg _))`/numbered-`1`/`it` shape that
//! upstream's `redundant_with_object?` node-matcher requires.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::BlockNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_EACH_WITH_OBJECT: &str = "Use `each` instead of `each_with_object`.";
const MSG_WITH_OBJECT: &str = "Remove redundant `with_object`.";

/// RuboCop's `(args (arg _))` / numbered-`1` / `it`-parameter block shapes:
/// exactly one plain required block parameter and nothing else, a numbered
/// block that only ever uses `_1`, or an `it`-parameter block.
fn has_single_object_param(block: &BlockNode<'_>) -> bool {
    let Some(params) = block.parameters() else { return false };
    match params.kind() {
        NodeKind::BlockParametersNode => {
            let bp = params.as_block_parameters_node().expect("kind matched");
            if !bp.locals().is_empty() {
                return false;
            }
            let Some(p) = bp.parameters() else { return false };
            p.requireds().len() == 1
                && p.requireds().first().is_some_and(|r| r.as_required_parameter_node().is_some())
                && p.optionals().is_empty()
                && p.rest().is_none()
                && p.posts().is_empty()
                && p.keywords().is_empty()
                && p.keyword_rest().is_none()
                && p.block().is_none()
        }
        NodeKind::NumberedParametersNode => {
            params.as_numbered_parameters_node().expect("kind matched").maximum() == 1
        }
        NodeKind::ItParametersNode => true,
        _ => false,
    }
}

/// Checks for redundant `with_object`.
#[derive(Debug, Clone)]
pub struct RedundantWithObject;

impl Rule for RedundantWithObject {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RedundantWithObject",
        department: Department::Lint,
        summary: "Checks for redundant `with_object`.",
        explanation: "\
Checks for redundant `with_object`.

```ruby
# bad
ary.each_with_object([]) do |v|
  v
end

# good
ary.each do |v|
  v
end

# bad
ary.each.with_object([]) do |v|
  v
end

# good
ary.each do |v|
  v
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
        let call = node.as_call_node().expect("kind matched");
        let name = call.name();
        let name = name.as_slice();
        let is_each_with_object = name == b"each_with_object";
        let is_with_object = name == b"with_object";
        if !is_each_with_object && !is_with_object {
            return;
        }

        // RuboCop's node-matcher requires exactly one positional argument
        // (the accumulator object) on the matched call.
        let Some(arguments) = call.arguments() else { return };
        if arguments.arguments().len() != 1 {
            return;
        }

        let Some(block_node) = call.block() else { return };
        let Some(block) = block_node.as_block_node() else { return };
        if !has_single_object_param(&block) {
            return;
        }

        let Some(message_loc) = call.message_loc() else { return };
        let end = call_span_excluding_block(&call).end;
        let range = Span::new(message_loc.span().start, end);
        let message = if is_each_with_object { MSG_EACH_WITH_OBJECT } else { MSG_WITH_OBJECT };

        let edits = if is_each_with_object {
            vec![Edit::replace(range, b"each".to_vec())]
        } else {
            let mut edits = vec![Edit::delete(range)];
            if let Some(operator_loc) = call.call_operator_loc() {
                edits.push(Edit::delete(operator_loc.span()));
            }
            edits
        };

        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}
