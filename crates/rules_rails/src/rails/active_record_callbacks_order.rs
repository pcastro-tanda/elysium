//! `Rails/ActiveRecordCallbacksOrder`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/active_record_callbacks_order.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::util::send_span;

const CALLBACKS_IN_ORDER: [&str; 19] = [
    "after_initialize",
    "before_validation",
    "after_validation",
    "before_save",
    "around_save",
    "before_create",
    "around_create",
    "after_create",
    "before_update",
    "around_update",
    "after_update",
    "before_destroy",
    "around_destroy",
    "after_destroy",
    "after_save",
    "after_commit",
    "after_rollback",
    "after_find",
    "after_touch",
];

/// Order callback declarations in the order in which they will be executed.
#[derive(Debug, Clone)]
pub struct ActiveRecordCallbacksOrder;

impl Rule for ActiveRecordCallbacksOrder {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ActiveRecordCallbacksOrder",
        department: Department::Rails,
        summary: "Order callback declarations in the order in which they will be executed.",
        explanation: "Checks that Active Record callbacks are declared in the order in which they \
                      will be executed.\n\n```ruby\n# bad\nclass Person < ApplicationRecord\n  \
                      after_commit :after_commit_callback\n  before_validation \
                      :before_validation_callback\nend\n\n# good\nclass Person < \
                      ApplicationRecord\n  before_validation :before_validation_callback\n  \
                      after_commit :after_commit_callback\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(class) = node.as_class_node() else { return };
        let Some(body) = class.body() else { return };
        let Some(statements) = body.as_statements_node() else { return };
        let children: Vec<Node<'_>> = statements.body().iter().collect();

        let mut previous_index: Option<usize> = None;
        let mut previous_callback: &[u8] = b"";
        for (i, child) in children.iter().enumerate() {
            let Some((name, index)) = callback(child) else { continue };
            if previous_index.is_some_and(|p| index < p) {
                let message = format!(
                    "`{}` is supposed to appear before `{}`.",
                    String::from_utf8_lossy(name),
                    String::from_utf8_lossy(previous_callback)
                );
                let call = child.as_call_node().expect("callback is a call");
                let span = send_span(&call);
                let previous = children[..i].iter().rev().find(|c| callback(c).is_some());
                let fix = previous.map(|previous| {
                    let current_range = source_range_with_comment(ctx, child);
                    let previous_range = source_range_with_comment(ctx, previous);
                    let text = ctx.text(current_range).to_vec();
                    Fix {
                        applicability: Applicability::Safe,
                        edits: vec![
                            Edit::insert(previous_range.start, text),
                            Edit::delete(current_range),
                        ],
                    }
                });
                match fix {
                    Some(fix) => ctx.report_with_fix(&Self::META, span, message, fix),
                    None => ctx.report(&Self::META, span, message),
                }
            }
            previous_index = Some(index);
            previous_callback = name;
        }
    }
}

/// `node.send_type? && CALLBACKS_ORDER_MAP.key?(node.method_name)`.
fn callback<'a>(node: &'a Node<'_>) -> Option<(&'a [u8], usize)> {
    let call: CallNode<'_> = node.as_call_node()?;
    if call.is_safe_navigation() || call.block().is_some_and(|b| b.as_block_node().is_some()) {
        return None;
    }
    let name = call.name().as_slice();
    let index = CALLBACKS_IN_ORDER.iter().position(|c| c.as_bytes() == name)?;
    // SAFETY-free lifetime tie: re-borrow the name from the static table.
    Some((CALLBACKS_IN_ORDER[index].as_bytes(), index))
}

fn source_range_with_comment(ctx: &Context<'_>, node: &Node<'_>) -> Span {
    let begin = begin_pos_with_comment(ctx, node);
    let end_line = ctx.line_col(node.span().end).line;
    Span::new(begin, ctx.line_span(end_line).end)
}

fn begin_pos_with_comment(ctx: &Context<'_>, node: &Node<'_>) -> u32 {
    let mut first_line = ctx.line_col(node.span().start).line;
    let mut annotation_line = first_line.saturating_sub(1);
    while annotation_line > 0
        && ctx.comments().iter().any(|c| {
            c.line == annotation_line
                && ctx.line_text(annotation_line).trim_ascii_start().starts_with(b"#")
        })
    {
        first_line = annotation_line;
        annotation_line -= 1;
    }
    ctx.line_span(first_line).start.saturating_sub(1)
}
