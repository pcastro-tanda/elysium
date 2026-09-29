//! `Lint/RedundantSplatExpansion`, ported from RuboCop's
//! `lib/rubocop/cop/lint/redundant_splat_expansion.rb`.
//!
//! Whitequark/Prism shape differences this port has to bridge:
//!
//! - A splat's expanded item is matched by one of three whitequark shapes:
//!   a literal (`str`/`dstr`/`int`/`float`/`array`), a bare `Array.new(...)`
//!   `send`, or a `block`-wrapped `Array.new(...) { ... }` whose captured
//!   node is the *inner* `send`. Prism has no separate block-wrapper node --
//!   `CallNode::block` is just a field on the same `CallNode` -- so both
//!   `send` shapes collapse into "the expression is a `CallNode` matching
//!   `Array.new`/`::Array.new`", checked via `CallNode::block().is_some()`
//!   only where upstream's *nesting* (not its `send_type?` branch) would
//!   otherwise matter (see `array_new_inside_array_literal?` below).
//! - `array_new_inside_array_literal?` walks two whitequark parents up from
//!   the `send`: through the `block` wrapper (if any) to the splat, then to
//!   the splat's own parent. In Prism there is no wrapper level, so a
//!   block-having call's grandparent-per-whitequark is the splat node
//!   itself (never an array), which can never satisfy the "array with more
//!   than one child" test -- i.e. that suppression path is simply unreachable
//!   whenever a block is attached, block or no. This is reproduced literally
//!   by only ever running the check for the no-block shape.
//! - Whitequark wraps a single-value assignment's splat RHS (`a = *x`,
//!   `a, b = *x`) in an implicit `array` node with no `begin`/`end` locations;
//!   Prism does the same (an `ArrayNode` with no `opening_loc`/`closing_loc`
//!   whose span exactly matches its lone `SplatNode` child) -- no bridging
//!   needed there. But whitequark *also* wraps a `rescue`'s exception list in
//!   such an implicit `array` (so `grandparent.resbody_type?` reaches two
//!   levels up, past that wrapper), whereas Prism's `RescueNode::exceptions`
//!   holds the splat directly with no wrapper at all; reproduced by checking
//!   the splat's *immediate* Prism parent for `RescueNode` instead.
//! - Call arguments are direct children of whitequark's `send` node, but
//!   Prism interposes its own `ArgumentsNode` between a `CallNode` and its
//!   arguments; skipped transparently by `logical_parent`/`logical_grandparent`
//!   wherever upstream inspects `node.parent`/`node.parent.parent` looking
//!   for a `send`/assignment node.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    NodeInfo, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ArrayNode, CallNode};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Replace splat expansion with comma separated values.";
/// RuboCop's `ARRAY_PARAM_MSG`.
const ARRAY_PARAM_MSG: &str = "Pass array contents as separate arguments.";

/// Checks for splat unnecessarily being called on literals.
#[derive(Debug, Clone)]
pub struct RedundantSplatExpansion {
    allow_percent_literal_array_argument: bool,
    /// Stack of `ArrayNode::elements().len()`, pushed on entering an
    /// `ArrayNode` and popped on leaving it. Whenever the splat currently
    /// being visited sits directly inside an array literal, the top of this
    /// stack is that array's element count -- `NodeInfo` (kind + span only)
    /// cannot answer that on its own.
    array_element_stack: Vec<usize>,
}

impl Rule for RedundantSplatExpansion {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RedundantSplatExpansion",
        department: Department::Lint,
        summary: "Checks for splat unnecessarily being called on literals.",
        explanation: "\
Checks for unneeded usages of splat expansion.

```ruby
# bad
a = *[1, 2, 3]
['a', 'b', *%w(c d e), 'f', 'g']

# good
c = [1, 2, 3]
a = *c
a = *1..10

# bad
do_something(*['foo', 'bar', 'baz'])

# good
do_something('foo', 'bar', 'baz')

# bad
case foo
when *[1, 2, 3]
  bar
end

# good
case foo
when 1, 2, 3
  bar
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::SplatNode, NodeKind::ArrayNode],
        config: &[ConfigOption {
            name: "AllowPercentLiteralArrayArgument",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Allows a percent literal array being used as a method argument (`do_something(*%w[foo bar baz])`).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_percent_literal_array_argument: options.bool("AllowPercentLiteralArrayArgument"),
            array_element_stack: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(array) = node.as_array_node() {
            self.array_element_stack.push(array.elements().len());
            return;
        }

        let Some(splat) = node.as_splat_node() else { return };
        let Some(expression) = splat.expression() else { return };

        let array_new_call = is_array_new_call(&expression);

        let is_literal_shape = matches!(
            expression.kind(),
            NodeKind::StringNode
                | NodeKind::InterpolatedStringNode
                | NodeKind::IntegerNode
                | NodeKind::FloatNode
                | NodeKind::ArrayNode
        );

        if is_literal_shape {
            if let Some(arr) = expression.as_array_node() {
                if arr.elements().is_empty() {
                    // An empty array/percent literal (`*[]`, `*%w()`, ...)
                    // expands to nothing, so removing the splat would
                    // produce invalid or semantically different code.
                    return;
                }
            }
        } else if let Some(call) = &array_new_call {
            if self.array_new_expansion_suppressed(ctx, node.span(), call) {
                return;
            }
        } else {
            return;
        }

        let raw_parent = ctx.parent();
        let is_method_argument = logical_parent(ctx.ancestors(), node.span())
            .is_some_and(|p| p.kind == NodeKind::CallNode);
        let is_part_of_array = raw_parent
            .is_some_and(|p| p.kind == NodeKind::ArrayNode && array_has_explicit_brackets(ctx, p));
        let is_array_literal_expr = expression.kind() == NodeKind::ArrayNode;

        let mut message = MSG;
        if is_array_literal_expr && (is_method_argument || is_part_of_array) {
            if self.allow_percent_literal_array_argument
                && is_method_argument
                && is_percent_literal_array(&expression)
            {
                return;
            }
            message = ARRAY_PARAM_MSG;
        }

        let operator_span = splat.operator_loc().span();
        let (range, replacement) =
            replacement_range_and_content(ctx, node.span(), operator_span, &expression, raw_parent);
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(range, replacement)],
        };
        ctx.report_with_fix(&Self::META, node.span(), message, fix);
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.as_array_node().is_some() {
            self.array_element_stack.pop();
        }
    }
}

impl RedundantSplatExpansion {
    /// RuboCop's `redundant_splat_expansion`, restricted to the `Array.new`
    /// (`send_type?`) branch: `array_new_inside_array_literal?` plus the
    /// `ASSIGNMENT_TYPES` grandparent check.
    fn array_new_expansion_suppressed(
        &self,
        ctx: &Context<'_>,
        splat_span: Span,
        call: &CallNode<'_>,
    ) -> bool {
        if call.block().is_none() {
            let raw_parent = ctx.parent();
            if raw_parent.is_some_and(|p| p.kind == NodeKind::ArrayNode)
                && self.array_element_stack.last().is_some_and(|&count| count > 1)
            {
                return true;
            }
        }
        if let Some(grandparent) = logical_grandparent(ctx.ancestors(), splat_span) {
            if !is_assignment_kind(grandparent.kind) {
                return true;
            }
        }
        false
    }
}

/// RuboCop's `array_new?` node matcher: `Array.new(...)`/`::Array.new(...)`,
/// with or without a trailing block.
fn is_array_new_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() || call.name().as_slice() != b"new" {
        return None;
    }
    let receiver = call.receiver()?;
    if !ext::is_bare_or_toplevel_const(&receiver) {
        return None;
    }
    if ext::const_name(&receiver)?.as_str() != "Array" {
        return None;
    }
    Some(call)
}

/// RuboCop's `ASSIGNMENT_TYPES` (`lvasgn ivasgn cvasgn gvasgn casgn`).
fn is_assignment_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
    )
}

/// `node.parent`/`node.parent.parent`, bridged across two whitequark/Prism
/// shape gaps: an interposed `ArgumentsNode` (absent from whitequark, where
/// a call's arguments are its own direct children), and a `StatementsNode`
/// that holds only one statement (whitequark elides a single-statement
/// `begin`, so its "parent" skips straight past where Prism always wraps
/// one). Once the walk reaches the top (`ProgramNode`), whitequark has no
/// equivalent node at all -- its top-level parse result is the lone
/// statement itself when there is only one -- so that is reported as no
/// parent (`None`), not as `ProgramNode`.
fn logical_ancestor(ancestors: &[NodeInfo], node_span: Span, levels: usize) -> Option<NodeInfo> {
    let mut current_span = node_span;
    let mut idx = ancestors.len();
    let mut remaining = levels;
    loop {
        idx = idx.checked_sub(1)?;
        let info = ancestors[idx];
        let transparent = info.kind == NodeKind::ArgumentsNode
            || (info.kind == NodeKind::StatementsNode && info.span == current_span);
        current_span = info.span;
        if transparent {
            continue;
        }
        remaining -= 1;
        if remaining == 0 {
            return (info.kind != NodeKind::ProgramNode).then_some(info);
        }
    }
}

fn logical_parent(ancestors: &[NodeInfo], node_span: Span) -> Option<NodeInfo> {
    logical_ancestor(ancestors, node_span, 1)
}

fn logical_grandparent(ancestors: &[NodeInfo], node_span: Span) -> Option<NodeInfo> {
    logical_ancestor(ancestors, node_span, 2)
}

/// Whether an `ArrayNode` ancestor known only as a `NodeInfo` (kind + span)
/// was written with explicit brackets (`[...]`/`%w(...)`/...) rather than
/// being whitequark/Prism's implicit wrapper around a splat assignment's
/// value (`a = *x`), which has no `begin`/`end` location. Every explicit
/// array literal's span starts with `[` or `%`; the implicit wrapper's span
/// starts wherever its lone child (the splat) starts.
fn array_has_explicit_brackets(ctx: &Context<'_>, info: NodeInfo) -> bool {
    let first_byte = ctx.text(Span::new(info.span.start, info.span.start + 1)).first().copied();
    matches!(first_byte, Some(b'[' | b'%'))
}

/// RuboCop's `use_percent_literal_array_argument?`'s literal check
/// (`percent_literal?(:string) || percent_literal?(:symbol)`), i.e. any of
/// `%w`/`%W`/`%i`/`%I`.
fn is_percent_literal_array(expression: &Node<'_>) -> bool {
    expression.as_array_node().and_then(|arr| arr.opening_loc()).is_some_and(|loc| {
        let bytes = loc.as_slice();
        bytes.len() >= 2 && bytes[0] == b'%' && matches!(bytes[1], b'w' | b'W' | b'i' | b'I')
    })
}

/// RuboCop's `replacement_range_and_content`.
fn replacement_range_and_content(
    ctx: &Context<'_>,
    splat_span: Span,
    operator_span: Span,
    expression: &Node<'_>,
    raw_parent: Option<NodeInfo>,
) -> (Span, Vec<u8>) {
    if is_array_new_call(expression).is_some() {
        let range = if raw_parent.is_some_and(|p| p.kind == NodeKind::ArrayNode) {
            raw_parent.expect("checked above").span
        } else {
            splat_span
        };
        return (range, ctx.text(expression.span()).to_vec());
    }

    if expression.kind() != NodeKind::ArrayNode {
        let mut replacement = ctx.text(expression.span()).to_vec();
        let wrap = raw_parent
            .is_some_and(|p| p.kind == NodeKind::ArrayNode && !array_has_explicit_brackets(ctx, p));
        if wrap {
            let mut wrapped = Vec::with_capacity(replacement.len() + 2);
            wrapped.push(b'[');
            wrapped.append(&mut replacement);
            wrapped.push(b']');
            replacement = wrapped;
        }
        return (splat_span, replacement);
    }

    if redundant_brackets(ctx, raw_parent, ctx.ancestors(), splat_span) {
        let array = expression.as_array_node().expect("checked array kind above");
        return (splat_span, remove_brackets(ctx, &array));
    }

    (operator_span, Vec::new())
}

/// RuboCop's `redundant_brackets?`.
fn redundant_brackets(
    ctx: &Context<'_>,
    raw_parent: Option<NodeInfo>,
    ancestors: &[NodeInfo],
    splat_span: Span,
) -> bool {
    if raw_parent.is_some_and(|p| p.kind == NodeKind::WhenNode) {
        return true;
    }
    if logical_parent(ancestors, splat_span).is_some_and(|p| p.kind == NodeKind::CallNode) {
        return true;
    }
    if raw_parent
        .is_some_and(|p| p.kind == NodeKind::ArrayNode && array_has_explicit_brackets(ctx, p))
    {
        return true;
    }
    // Whitequark wraps a `rescue`'s exception list in an implicit `array`
    // node, so `grandparent.resbody_type?` there is really asking "is the
    // splat's array-wrapper's parent a `resbody`?"; Prism's `RescueNode`
    // holds the splat directly with no such wrapper.
    raw_parent.is_some_and(|p| p.kind == NodeKind::RescueNode)
}

/// RuboCop's `remove_brackets`.
fn remove_brackets(ctx: &Context<'_>, array: &ArrayNode<'_>) -> Vec<u8> {
    let elements: Vec<String> = array
        .elements()
        .iter()
        .map(|element| String::from_utf8_lossy(ctx.text(element.span())).into_owned())
        .collect();
    let opening = array.opening_loc().map_or(b"[".as_slice(), |loc| loc.as_slice());

    let joined = if opening.starts_with(b"%w") {
        format!("'{}'", elements.join("', '"))
    } else if opening.starts_with(b"%W") {
        format!("\"{}\"", elements.join("\", \""))
    } else if opening.starts_with(b"%i") {
        format!(":{}", elements.join(", :"))
    } else if opening.starts_with(b"%I") {
        format!(":\"{}\"", elements.join("\", :\""))
    } else {
        elements.join(", ")
    };
    joined.into_bytes()
}
