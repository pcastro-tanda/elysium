//! `Rails/WhereMissing`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/where_missing.rb`.
//!
//! RuboCop's parent navigation (`node.parent`, `right_sibling`) is rebuilt
//! from a root-to-node path, found once per `left_joins` call. A call with a
//! literal block is a `block` node in whitequark's tree, so the call itself
//! (without its block) is where a climb up the receiver chain stops.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `minimum_target_rails_version 6.1`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 6.1;

/// Use `where.missing(...)` to find missing relationship records.
#[derive(Debug, Clone)]
pub struct WhereMissing {
    supported: bool,
}

impl Rule for WhereMissing {
    const META: RuleMeta = RuleMeta {
        name: "Rails/WhereMissing",
        department: Department::Rails,
        summary: "Use `where.missing(...)` to find missing relationship records.",
        explanation: "Use `where.missing(...)` to find missing relationship records.\n\nThis cop \
                      is enabled in Rails 6.1 or higher.\n\n```ruby\n# bad\nPost.left_joins(:author)\
                      .where(authors: { id: nil })\n\n# good\nPost.where.missing(:author)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation()
            || !matches!(call.name().as_slice(), b"left_joins" | b"left_outer_joins")
        {
            return;
        }
        let Some(first) = call.arguments().and_then(|args| args.arguments().first()) else {
            return;
        };
        let Some(joined) = first.as_symbol_node() else { return };
        let joined = joined.unescaped().to_vec();

        let root = ctx.parsed().root();
        let mut path = Vec::new();
        if !find_path(&root, node.span(), &mut path) {
            return;
        }
        let root_index = root_receiver(&path, path.len() - 1);
        let root_text = send_text(ctx, &path[root_index]);

        let mut stack = path[..=root_index].to_vec();
        let mut found = None;
        walk(&mut stack, true, &mut |stack| {
            let Some(where_node) = stack.last() else { return false };
            let Some((where_call, argument)) = where_node_and_argument(where_node) else {
                return false;
            };
            let index = root_receiver(stack, stack.len() - 1);
            if send_text(ctx, &stack[index]) != root_text || !same_relationship(&argument, &joined)
            {
                return false;
            }
            found = Some((*where_node, where_call, argument));
            true
        });
        let Some((where_node, where_call, argument)) = found else { return };

        let Some(selector) = call.message_loc() else { return };
        let node_end = call_span_excluding_block(&call).end;
        let range = Span::new(selector.span().start, node_end);
        let method = String::from_utf8_lossy(call.name().as_slice()).into_owned();
        let joined_name = String::from_utf8_lossy(&joined).into_owned();
        let message = format!(
            "Use `where.missing(:{joined_name})` instead of \
             `{method}(:{joined_name}).where({}: {{ id: nil }})`.",
            String::from_utf8_lossy(&argument)
        );

        let mut edits = vec![Edit::replace(selector.span(), b"where.missing".to_vec())];
        let multi_condition = where_call
            .arguments()
            .and_then(|args| args.arguments().first())
            .is_some_and(|hash| hash_elements(&hash).is_some_and(|elements| elements.len() > 1));
        if multi_condition {
            let mut hash_stack = vec![where_node];
            walk(&mut hash_stack, true, &mut |stack| {
                let Some(elements) = stack.last().and_then(hash_elements) else { return false };
                for (index, element) in elements.iter().enumerate() {
                    if !is_missing_relationship(element) {
                        continue;
                    }
                    let span = element.span();
                    let remove = if let Some(next) = elements.get(index + 1) {
                        Span::new(span.start, next.span().start)
                    } else if index > 0 {
                        Span::new(elements[index - 1].span().end, span.end)
                    } else {
                        span
                    };
                    edits.push(Edit::replace(remove, Vec::new()));
                }
                false
            });
        } else {
            let Some(where_selector) = where_call.message_loc() else { return };
            let where_end = where_call
                .closing_loc()
                .map_or_else(|| call_span_excluding_block(&where_call).end, |loc| loc.span().end);
            let mut remove = Span::new(where_selector.span().start, where_end);
            let multiline = ctx.line_col(node.span().start).line
                != ctx.line_col(node_end.saturating_sub(1).max(selector.span().start)).line;
            let same_line = ctx.same_line(selector.span(), where_selector.span());
            if multiline && !same_line {
                remove = ctx.whole_lines(remove);
            } else if let Some(dot) = where_call.call_operator_loc() {
                edits.push(Edit::replace(dot.span(), Vec::new()));
            } else if let Some(dot) = call.call_operator_loc() {
                edits.push(Edit::replace(dot.span(), Vec::new()));
            }
            edits.push(Edit::replace(remove, Vec::new()));
        }

        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// Depth-first search for the `CallNode` spanning `target`, recording the
/// root-to-node path.
fn find_path<'pr>(node: &Node<'pr>, target: Span, path: &mut Vec<Node<'pr>>) -> bool {
    let span = node.span();
    if span.start > target.start || span.end < target.end {
        return false;
    }
    path.push(*node);
    if node.kind() == NodeKind::CallNode && span == target {
        return true;
    }
    let mut children = Vec::new();
    for_each_child(node, |child| children.push(*child));
    for child in &children {
        if find_path(child, target, path) {
            return true;
        }
    }
    path.pop();
    false
}

/// `root_receiver`: climbs through `send` parents (the node being a direct
/// child) until a block, a non-send, or an `or`/`and` call. Returns the index
/// in `path` of the node it stops at.
fn root_receiver(path: &[Node<'_>], mut index: usize) -> usize {
    loop {
        let Some(call) = path[index].as_call_node() else { return index };
        if call.block().is_some_and(|block| block.as_block_node().is_some()) {
            return index;
        }
        let Some(mut parent) = index.checked_sub(1) else { return index };
        if path[parent].kind() == NodeKind::ArgumentsNode {
            let Some(grandparent) = parent.checked_sub(1) else { return index };
            parent = grandparent;
        }
        let Some(parent_call) = path[parent].as_call_node() else { return index };
        if parent_call.is_safe_navigation()
            || matches!(parent_call.name().as_slice(), b"or" | b"and")
        {
            return index;
        }
        index = parent;
    }
}

/// The text of a send without any block attached to it.
fn send_text(ctx: &Context<'_>, node: &Node<'_>) -> Vec<u8> {
    let span =
        node.as_call_node().map_or_else(|| node.span(), |call| call_span_excluding_block(&call));
    ctx.text(span).to_vec()
}

/// Pre-order walk of `stack.last()`'s subtree as whitequark sees it: the
/// top node's attached block is not part of its `send`. Stops when `visit`
/// returns `true`.
fn walk<'pr>(
    stack: &mut Vec<Node<'pr>>,
    top: bool,
    visit: &mut dyn FnMut(&Vec<Node<'pr>>) -> bool,
) -> bool {
    if visit(stack) {
        return true;
    }
    let Some(node) = stack.last().copied() else { return false };
    let mut children = Vec::new();
    for_each_child(&node, |child| children.push(*child));
    for child in children {
        if top && child.kind() == NodeKind::BlockNode && node.as_call_node().is_some() {
            continue;
        }
        stack.push(child);
        if walk(stack, false, visit) {
            return true;
        }
        stack.pop();
    }
    false
}

fn hash_elements<'pr>(node: &Node<'pr>) -> Option<Vec<Node<'pr>>> {
    if let Some(hash) = node.as_keyword_hash_node() {
        Some(hash.elements().iter().collect())
    } else {
        Some(node.as_hash_node()?.elements().iter().collect())
    }
}

/// `$(send ... :where (hash <(pair $(sym _) (hash (pair (sym :id) (nil))))
/// ...>))`: the `where` call and the unescaped key of the first qualifying
/// pair.
fn where_node_and_argument<'pr>(node: &Node<'pr>) -> Option<(CallNode<'pr>, Vec<u8>)> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"where" || call.is_safe_navigation() {
        return None;
    }
    let arguments: Vec<Node<'pr>> = call.arguments()?.arguments().iter().collect();
    let [argument] = arguments.as_slice() else { return None };
    let elements = hash_elements(argument)?;
    for element in &elements {
        if let Some(key) = missing_relationship_key(element) {
            return Some((call, key));
        }
    }
    None
}

/// `(pair (sym _) (hash (pair (sym :id) (nil))))`, returning the first key.
fn missing_relationship_key(node: &Node<'_>) -> Option<Vec<u8>> {
    let pair = node.as_assoc_node()?;
    let key = pair.key();
    let key = key.as_symbol_node()?;
    let value = pair.value();
    let inner = value.as_hash_node()?;
    let inner_elements: Vec<Node<'_>> = inner.elements().iter().collect();
    let [inner_pair] = inner_elements.as_slice() else { return None };
    let inner_pair = inner_pair.as_assoc_node()?;
    let inner_key = inner_pair.key();
    if inner_key.as_symbol_node()?.unescaped() != b"id"
        || inner_pair.value().as_nil_node().is_none()
    {
        return None;
    }
    Some(key.unescaped().to_vec())
}

fn is_missing_relationship(node: &Node<'_>) -> bool {
    missing_relationship_key(node).is_some()
}

/// `where.value.to_s.match?(/^#{left_joins.value}s?$/)`.
fn same_relationship(where_key: &[u8], joined: &[u8]) -> bool {
    let (Ok(where_key), Ok(joined)) = (std::str::from_utf8(where_key), std::str::from_utf8(joined))
    else {
        return false;
    };
    Regex::new(&format!("(?m)^{joined}s?$")).is_ok_and(|regex| regex.is_match(where_key))
}
