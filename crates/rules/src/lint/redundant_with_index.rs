//! `Lint/RedundantWithIndex`, ported from RuboCop's
//! `lib/rubocop/cop/lint/redundant_with_index.rb`.
//!
//! RuboCop's `on_block`/`on_numblock`/`on_itblock` all fire on whitequark's
//! unified block-with-its-call node; Prism instead attaches a literal block
//! to the [`ruby_ast::node::CallNode`] it belongs to via `call.block()`, so
//! this rule dispatches on [`NodeKind::CallNode`] directly and inspects the
//! attached block for the `(args (arg _))`/numbered-`1`/`it` shape that
//! upstream's `redundant_with_index?` node-matcher requires.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::{BlockNode, CallNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_EACH_WITH_INDEX: &str = "Use `each` instead of `each_with_index`.";
const MSG_WITH_INDEX: &str = "Remove redundant `with_index`.";

/// RuboCop's `(args (arg _))` / numbered-`1` / `it`-parameter block shapes:
/// exactly one plain required block parameter and nothing else, a numbered
/// block that only ever uses `_1`, or an `it`-parameter block.
fn has_single_index_param(block: &BlockNode<'_>) -> bool {
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

/// RuboCop's two guards ahead of the node-matcher: `return unless
/// node.receiver` (a receiverless `with_index { }`/`each_with_index { }`
/// never matches), and `return if node.method?(:with_index) &&
/// !node.receiver.receiver` (a bare `with_index` chained straight off
/// another receiverless call, e.g. `ary.with_index { }`, is skipped since
/// dropping `with_index` there would not leave a valid call behind).
fn passes_receiver_guard(call: &CallNode<'_>, is_with_index: bool) -> bool {
    let Some(receiver) = call.receiver() else { return false };
    if is_with_index {
        return receiver.as_call_node().is_some_and(|r| r.receiver().is_some());
    }
    true
}

/// Checks for redundant `with_index`.
#[derive(Debug, Clone)]
pub struct RedundantWithIndex;

impl Rule for RedundantWithIndex {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RedundantWithIndex",
        department: Department::Lint,
        summary: "Checks for redundant `with_index`.",
        explanation: "\
Checks for redundant `with_index`.

```ruby
# bad
ary.each_with_index do |v|
  v
end

# good
ary.each do |v|
  v
end

# bad
ary.each.with_index do |v|
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
        let is_each_with_index = name == b"each_with_index";
        let is_with_index = name == b"with_index";
        if !is_each_with_index && !is_with_index {
            return;
        }

        let Some(block_node) = call.block() else { return };
        let Some(block) = block_node.as_block_node() else { return };
        if !passes_receiver_guard(&call, is_with_index) {
            return;
        }
        if !has_single_index_param(&block) {
            return;
        }

        let Some(message_loc) = call.message_loc() else { return };
        let end = call_span_excluding_block(&call).end;
        let range = Span::new(message_loc.span().start, end);
        let message = if is_each_with_index { MSG_EACH_WITH_INDEX } else { MSG_WITH_INDEX };

        let edits = if is_each_with_index {
            vec![Edit::replace(message_loc.span(), b"each".to_vec())]
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
