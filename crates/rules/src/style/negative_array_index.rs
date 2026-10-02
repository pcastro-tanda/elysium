//! `Style/NegativeArrayIndex`, ported from RuboCop's
//! `lib/rubocop/cop/style/negative_array_index.rb`.
//!
//! whitequark's generic `Node#receiver` (defined on the base `Node` class via
//! a `def_node_matcher` pattern, `{(send $_ ...) (any_block (call $_ ...)
//! ...)}`) returns `nil` for any node that is not itself a method call with
//! an explicit receiver slot (a bare identifier/ivar/cvar/const/self node
//! included) -- it never raises. [`node_receiver`] mirrors that: `None`
//! unless `node` is a [`NodeKind::CallNode`] (whose own `receiver()` may
//! itself be absent, for an implicit-`self` call like bare `size`).
//!
//! whitequark's `begin_type?` (an explicit `(...)` grouping) is Prism's
//! [`NodeKind::ParenthesesNode`]; its single-statement `StatementsNode` body
//! is unwrapped the same way throughout (`extract_range_from_begin`,
//! `extract_inner_end`, and the simple pattern's `index_arg.begin_type?`
//! checks, all ported as direct `ParenthesesNode` matches).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::RangeNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `%<receiver>s[-%<index>s]` instead of `%<current>s`.";
const MSG_RANGE: &str =
    "Use `%<receiver>s[%<start>s%<range_op>s-%<index>s]` instead of `%<current>s`.";

const PRESERVING_METHODS: &[&[u8]] = &[b"sort", b"reverse", b"shuffle", b"rotate"];

/// Use negative array indices instead of calculating array length minus a
/// value. Also handles range patterns with length calculations. Recognizes
/// preserving methods and their combinations, allowing safe replacement when
/// the receiver matches.
#[derive(Debug, Clone)]
pub struct NegativeArrayIndex;

impl Rule for NegativeArrayIndex {
    const META: RuleMeta = RuleMeta {
        name: "Style/NegativeArrayIndex",
        department: Department::Style,
        summary: "Use negative array indices instead of calculating array length minus a value. \
            Also handles range patterns with length calculations. Recognizes preserving methods \
            and their combinations, allowing safe replacement when the receiver matches.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::IndexAndWriteNode,
            NodeKind::IndexOrWriteNode,
            NodeKind::IndexOperatorWriteNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (receiver, args) = match node {
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                if call.name().as_slice() != b"[]" {
                    return;
                }
                (call.receiver(), call.arguments())
            }
            Node::IndexAndWriteNode { .. } => {
                let n = node.as_index_and_write_node().expect("kind matched");
                (n.receiver(), n.arguments())
            }
            Node::IndexOrWriteNode { .. } => {
                let n = node.as_index_or_write_node().expect("kind matched");
                (n.receiver(), n.arguments())
            }
            Node::IndexOperatorWriteNode { .. } => {
                let n = node.as_index_operator_write_node().expect("kind matched");
                (n.receiver(), n.arguments())
            }
            _ => return,
        };
        let Some(args) = args else { return };
        if args.arguments().is_empty() {
            return;
        }
        let index_arg = args.arguments().iter().next().expect("checked non-empty");

        let range_candidate = extract_range_from_begin(&index_arg);
        if let (Some(range_node), Some(array_receiver)) =
            (range_candidate.as_range_node(), receiver)
        {
            if range_with_length_subtraction(ctx, &range_node, &array_receiver) {
                handle_range_pattern(ctx, &array_receiver, &range_node, &index_arg);
                return;
            }
        }

        handle_simple_index_pattern(ctx, receiver, &index_arg);
    }
}

/// Generic `Node#receiver`: `None` unless `node` is itself a method call
/// with an explicit receiver slot.
fn node_receiver<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    node.as_call_node().and_then(|c| c.receiver())
}

/// RuboCop's `length_subtraction?`: `(send (send $_ {:length :size :count})
/// :- (int $_))`.
fn length_subtraction<'pr>(node: &Node<'pr>) -> Option<(Option<Node<'pr>>, i32)> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"-" {
        return None;
    }
    let args: Vec<Node<'_>> = call.arguments()?.arguments().iter().collect();
    let [arg] = args.as_slice() else { return None };
    let value: i32 = arg.as_integer_node()?.value().try_into().ok()?;

    let receiver = call.receiver()?;
    let inner = receiver.as_call_node()?;
    if !matches!(inner.name().as_slice(), b"length" | b"size" | b"count") {
        return None;
    }
    if inner.arguments().is_some() {
        return None;
    }
    Some((inner.receiver(), value))
}

/// RuboCop's `extract_range_from_begin`: `node.begin_type? ?
/// node.children.first : node`.
fn extract_range_from_begin<'pr>(node: &Node<'pr>) -> Node<'pr> {
    if let Some(parens) = node.as_parentheses_node() {
        if let Some(first) =
            parens.body().and_then(|b| b.as_statements_node()).and_then(|s| s.body().iter().next())
        {
            return first;
        }
    }
    *node
}

/// RuboCop's `extract_inner_end`: `node.children.size == 1 ?
/// node.children.first : node`.
fn extract_inner_end<'pr>(node: &Node<'pr>) -> Node<'pr> {
    extract_range_from_begin(node)
}

/// RuboCop's `preserving_method?`.
fn preserving_method(node: &Node<'_>) -> bool {
    let Some(receiver) = node_receiver(node) else { return true };
    let call = node.as_call_node().expect("node_receiver implies a call node");
    if !PRESERVING_METHODS.contains(&call.name().as_slice()) {
        return false;
    }
    preserving_method(&receiver)
}

/// RuboCop's `extract_base_receiver`.
fn extract_base_receiver<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let receiver = node_receiver(node)?;
    if node_receiver(&receiver).is_none() {
        Some(receiver)
    } else {
        extract_base_receiver(&receiver)
    }
}

/// RuboCop's `receivers_match?`.
fn receivers_match(
    ctx: &Context<'_>,
    length_receiver: Option<&Node<'_>>,
    array_receiver: &Node<'_>,
) -> bool {
    let Some(length_receiver) = length_receiver else {
        return array_receiver.as_self_node().is_some();
    };
    if !preserving_method(array_receiver) || !preserving_method(length_receiver) {
        return false;
    }
    if ctx.text(length_receiver.span()) == ctx.text(array_receiver.span()) {
        return true;
    }
    extract_base_receiver(array_receiver).is_some()
}

/// RuboCop's `receivers_match_strict?`.
fn receivers_match_strict(
    ctx: &Context<'_>,
    length_receiver: Option<&Node<'_>>,
    array_receiver: &Node<'_>,
) -> bool {
    let Some(length_receiver) = length_receiver else { return false };
    preserving_method(array_receiver)
        && ctx.text(length_receiver.span()) == ctx.text(array_receiver.span())
}

/// RuboCop's `range_with_length_subtraction?`.
fn range_with_length_subtraction(
    ctx: &Context<'_>,
    range_node: &RangeNode<'_>,
    array_receiver: &Node<'_>,
) -> bool {
    let (Some(range_end), Some(range_start)) = (range_node.right(), range_node.left()) else {
        return false;
    };
    if !preserving_method(&range_start) {
        return false;
    }
    let inner_end = extract_inner_end(&range_end);
    let Some((length_receiver, negative_index)) = length_subtraction(&inner_end) else {
        return false;
    };
    if negative_index <= 0 {
        return false;
    }
    receivers_match_strict(ctx, length_receiver.as_ref(), array_receiver)
}

/// RuboCop's `handle_simple_index_pattern`.
fn handle_simple_index_pattern(
    ctx: &mut Context<'_>,
    receiver: Option<Node<'_>>,
    index_arg: &Node<'_>,
) {
    let Some((length_receiver, negative_index)) = length_subtraction(index_arg) else { return };
    if negative_index <= 0 {
        return;
    }
    let Some(array_receiver) = receiver else { return };
    if !receivers_match(ctx, length_receiver.as_ref(), &array_receiver) {
        return;
    }
    add_offense_for_subtraction(ctx, &array_receiver, index_arg, negative_index);
}

/// RuboCop's `add_offense_for_subtraction`.
fn add_offense_for_subtraction(
    ctx: &mut Context<'_>,
    array_receiver: &Node<'_>,
    index_arg: &Node<'_>,
    negative_index: i32,
) {
    let receiver_src = String::from_utf8_lossy(ctx.text(array_receiver.span())).into_owned();
    let offense_range = index_arg.span();
    let index_src = String::from_utf8_lossy(ctx.text(offense_range)).into_owned();
    let current = format!("{receiver_src}[{index_src}]");
    let message = MSG
        .replacen("%<receiver>s", &receiver_src, 1)
        .replacen("%<index>s", &negative_index.to_string(), 1)
        .replacen("%<current>s", &current, 1);
    ctx.report_with_fix(
        &NegativeArrayIndex::META,
        offense_range,
        message,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(offense_range, format!("-{negative_index}").into_bytes())],
        },
    );
}

/// RuboCop's `handle_range_pattern` + `build_range_offense_data`.
fn handle_range_pattern(
    ctx: &mut Context<'_>,
    array_receiver: &Node<'_>,
    range_node: &RangeNode<'_>,
    index_arg: &Node<'_>,
) {
    let receiver_src = String::from_utf8_lossy(ctx.text(array_receiver.span())).into_owned();
    let range_end = range_node.right().expect("range_with_length_subtraction? checked");
    let range_start = range_node.left().expect("range_with_length_subtraction? checked");
    let inner_end = extract_inner_end(&range_end);
    let Some((_length_receiver, negative_index)) = length_subtraction(&inner_end) else { return };

    let range_op = if range_node.is_exclude_end() { "..." } else { ".." };
    let range_start_src = String::from_utf8_lossy(ctx.text(range_start.span())).into_owned();
    let has_parentheses = index_arg.as_parentheses_node().is_some();

    let end_expression = if range_end.as_parentheses_node().is_some() {
        String::from_utf8_lossy(ctx.text(range_end.span())).into_owned()
    } else {
        String::from_utf8_lossy(ctx.text(inner_end.span())).into_owned()
    };
    let range_without_parens = format!("{range_start_src}{range_op}{end_expression}");
    let current_source = if has_parentheses {
        format!("{receiver_src}[({range_without_parens})]")
    } else {
        format!("{receiver_src}[{range_without_parens}]")
    };

    let (start, index_display) = if has_parentheses {
        (format!("({range_start_src}"), format!("{negative_index})"))
    } else {
        (range_start_src.clone(), negative_index.to_string())
    };

    let message = MSG_RANGE
        .replacen("%<receiver>s", &receiver_src, 1)
        .replacen("%<start>s", &start, 1)
        .replacen("%<range_op>s", range_op, 1)
        .replacen("%<index>s", &index_display, 1)
        .replacen("%<current>s", &current_source, 1);

    let replacement = if has_parentheses {
        format!("({range_start_src}{range_op}-{negative_index})")
    } else {
        format!("{range_start_src}{range_op}-{negative_index}")
    };

    ctx.report_with_fix(
        &NegativeArrayIndex::META,
        range_end.span(),
        message,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(index_arg.span(), replacement.into_bytes())],
        },
    );
}
