//! `Rails/RedundantAllowNil`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/redundant_allow_nil.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_SAME: &str = "`allow_nil` is redundant when `allow_blank` has the same value.";
const MSG_ALLOW_NIL_FALSE: &str = "`allow_nil: false` is redundant when `allow_blank` is true.";

/// Checks Rails model validations for a redundant `allow_nil` when
/// `allow_blank` is present.
#[derive(Debug, Clone)]
pub struct RedundantAllowNil;

impl Rule for RedundantAllowNil {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RedundantAllowNil",
        department: Department::Rails,
        summary: "Finds redundant use of `allow_nil` when `allow_blank` is set to certain \
                  values in model validations.",
        explanation: "Checks Rails model validations for a redundant `allow_nil` when \
                      `allow_blank` is present.\n\n```ruby\n# bad\nvalidates :x, length: { is: \
                      5 }, allow_nil: true, allow_blank: true\n\n# bad\nvalidates :x, length: { \
                      is: 5 }, allow_nil: false, allow_blank: true\n\n# bad\nvalidates :x, \
                      length: { is: 5 }, allow_nil: false, allow_blank: false\n\n# good\n\
                      validates :x, length: { is: 5 }, allow_blank: true\n\n# good\nvalidates \
                      :x, length: { is: 5 }, allow_blank: false\n\n# good\n# Here, `nil` is \
                      valid but `''` is not\nvalidates :x, length: { is: 5 }, allow_nil: true, \
                      allow_blank: false\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Whether the two values have the same type is judged on Prism node kinds \
                      mapped onto whitequark's types.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"validates" || call.is_safe_navigation() {
            return;
        }
        let Some((allow_nil, allow_blank)) = find_allow_nil_and_allow_blank(node, true, ctx) else {
            return;
        };
        let (Some(nil_value), Some(blank_value)) =
            (allow_nil.value.as_ref(), allow_blank.value.as_ref())
        else {
            return;
        };
        let message = if wq_type(nil_value) == wq_type(blank_value) {
            MSG_SAME
        } else if nil_value.as_false_node().is_some() && blank_value.as_true_node().is_some() {
            MSG_ALLOW_NIL_FALSE
        } else {
            return;
        };

        let span = allow_nil.span;
        let removal = match (allow_nil.next_start, allow_nil.prev_end) {
            (Some(next_start), _) => Span::new(span.start, next_start),
            (None, Some(prev_end)) => Span::new(prev_end, span.end),
            (None, None) => span,
        };
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(removal)] },
        );
    }
}

/// A `pair` node and what its removal needs.
struct Pair<'pr> {
    span: Span,
    value: Option<Node<'pr>>,
    prev_end: Option<u32>,
    next_start: Option<u32>,
}

/// `find_allow_nil_and_allow_blank`.
fn find_allow_nil_and_allow_blank<'pr>(
    node: &Node<'pr>,
    root: bool,
    ctx: &Context<'_>,
) -> Option<(Pair<'pr>, Pair<'pr>)> {
    let mut children: Vec<Node<'pr>> = Vec::new();
    for_each_child(node, |child| {
        // A literal block belongs to the `block` node, not the `send`.
        if !(root && child.as_block_node().is_some()) {
            children.push(*child);
        }
    });
    // Whitequark has no `arguments` node: its children are the call's.
    let children = flatten_arguments(children);

    let mut allow_nil: Option<Pair<'pr>> = None;
    let mut allow_blank: Option<Pair<'pr>> = None;
    for (index, child) in children.iter().enumerate() {
        if let Some(assoc) = child.as_assoc_node() {
            let key = assoc.key();
            let key_source = pair_key_source(&key, ctx);
            let make = || Pair {
                span: child.span(),
                value: Some(assoc.value()),
                prev_end: index.checked_sub(1).map(|i| children[i].span().end),
                next_start: children.get(index + 1).map(|next| next.span().start),
            };
            match key_source.as_slice() {
                b"allow_nil" => allow_nil = Some(make()),
                b"allow_blank" => allow_blank = Some(make()),
                _ => {}
            }
        }
        if allow_nil.is_some() && allow_blank.is_some() {
            return allow_nil.zip(allow_blank);
        }
        if let Some(found) = find_allow_nil_and_allow_blank(child, false, ctx) {
            return Some(found);
        }
    }
    None
}

/// Replaces an `ArgumentsNode` child by its arguments.
fn flatten_arguments(children: Vec<Node<'_>>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    for child in children {
        if let Some(arguments) = child.as_arguments_node() {
            out.extend(arguments.arguments().iter());
        } else {
            out.push(child);
        }
    }
    out
}

/// `child_node.children.first.source`: a label key's source excludes the
/// colon.
fn pair_key_source(key: &Node<'_>, ctx: &Context<'_>) -> Vec<u8> {
    if let Some(symbol) = key.as_symbol_node() {
        let is_label = symbol.opening_loc().is_none() && symbol.closing_loc().is_some();
        if is_label {
            if let Some(value) = symbol.value_loc() {
                return ctx.text(value.span()).to_vec();
            }
        }
    }
    ctx.text(key.span()).to_vec()
}

/// The whitequark node type, as far as two values can be told apart.
fn wq_type(node: &Node<'_>) -> (NodeKind, bool) {
    match node.kind() {
        NodeKind::ConstantPathNode => (NodeKind::ConstantReadNode, false),
        NodeKind::UnlessNode => (NodeKind::IfNode, false),
        NodeKind::KeywordHashNode => (NodeKind::HashNode, false),
        NodeKind::CallNode => {
            (NodeKind::CallNode, node.as_call_node().is_some_and(|c| c.is_safe_navigation()))
        }
        kind => (kind, false),
    }
}
