//! `Style/AndOr`, ported from RuboCop's
//! `lib/rubocop/cop/style/and_or.rb`.
//!
//! # Node shapes
//!
//! Whitequark represents both `and`/`or` and `&&`/`||` as the same `and`/`or`
//! node types, distinguished only by `loc.operator`'s text
//! (`logical_operator?`); Prism does the same with [`NodeKind::AndNode`]/
//! [`NodeKind::OrNode`], so [`process_logical_operator`] below reads
//! `operator_loc`'s text the same way upstream's `PredicateOperatorNode`
//! mixin does.
//!
//! Whitequark's single `:if` node type covers `if`, `unless` and their
//! modifier forms (upstream's `on_if` therefore also sees every `unless`,
//! with no separate alias needed); Prism splits `unless` into its own
//! [`NodeKind::UnlessNode`], so this port subscribes to it directly
//! alongside `if`/`while`/`until` (`on_while`/`on_while_post`/`on_until`/
//! `on_until_post` upstream collapse to one Prism `WhileNode`/`UntilNode`
//! pair each, since Prism does not split the post-condition-loop form into
//! its own kind -- `is_begin_modifier()` distinguishes it, but both forms
//! expose the same `predicate()`).
//!
//! # `EnforcedStyle: conditionals`' condition scan
//!
//! Upstream's `on_conditionals` walks `node.condition.each_node(:and, :or)`:
//! every `and`/`or` node anywhere inside the condition subtree, however
//! deeply nested (through parenthesized groups, nested calls, ...), each
//! yielded once alongside its own true AST parent (`each_node` visits `self`
//! first, then recurses into every descendant regardless of intervening node
//! kinds). [`find_logical_operators`] mirrors this with a private walk built
//! on [`ruby_ast::for_each_child`] rather than [`Context::ancestors`],
//! because the latter only reflects the engine's own traversal position when
//! this rule's `enter` runs for the *outer* `if`/`unless`/`while`/`until`
//! node -- not the position of a node found by manually descending into its
//! condition. The parent kind this walk tracks is exactly what
//! [`keep_operator_precedence`] needs.
//!
//! # Autocorrection
//!
//! Naively swapping `and`/`or` for `&&`/`||` can silently change a
//! program's behaviour, since `&&`/`||` bind tighter than assignment,
//! `return`/`next`/`break`/`yield`, and (unlike `and`/`or`, which are
//! mutually equal-precedence and left-associative) `&&` binds tighter than
//! `||`. Upstream's corrector (mirrored in [`correct_child`] and friends)
//! wraps every operand that would otherwise re-associate, and
//! [`keep_operator_precedence`] additionally wraps whichever side of a mixed
//! `and`/`or` chain needs it to preserve the original grouping -- see the
//! four `*_precedes_*` fixtures for the concrete cases. This is why the cop
//! is unsafe (`fix: FixAvailability::Unsafe`), matching upstream's own
//! `@safety` note.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, Location, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Conditionals,
    Always,
}

/// Use &&/|| instead of and/or.
#[derive(Debug, Clone)]
pub struct AndOr {
    style: Style,
}

impl Rule for AndOr {
    const META: RuleMeta = RuleMeta {
        name: "Style/AndOr",
        department: Department::Style,
        summary: "Use &&/|| instead of and/or.",
        explanation: "\
Checks for uses of `and` and `or`, and suggests using `&&` and
`||` instead. It can be configured to check only in conditions or in
all contexts.

```ruby
# EnforcedStyle: conditionals (default)

# bad
if foo and bar
end

# good
foo.save && return

# good
foo.save and return

# good
if foo && bar
end
```

```ruby
# EnforcedStyle: always

# bad
foo.save and return

# bad
if foo and bar
end

# good
foo.save && return

# good
if foo && bar
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::AndNode,
            NodeKind::OrNode,
            NodeKind::IfNode,
            NodeKind::UnlessNode,
            NodeKind::WhileNode,
            NodeKind::UntilNode,
        ],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("conditionals"),
            allowed: &["conditionals", "always"],
            doc: "Whether `and`/`or` are banned only in conditionals (`conditionals`) or \
completely (`always`).",
        }],
        blind_spots: "\
Autocorrection is unsafe because there is a different operator precedence between logical
operators (`&&`/`||`) and semantic operators (`and`/`or`), and that might change behaviour --
inherited directly from upstream's own `@safety` note. RuboCop-AST's `parenthesized_call?` (used
to avoid re-wrapping an already-parenthesized `return`/`next`/`break` operand) is approximated as
always false for those three: Prism represents `return(x)` by wrapping `x` in its own
`ParenthesesNode` rather than giving the `return` node a paren location of its own, unlike
whitequark. `yield(x)`'s own `lparen_loc` is checked directly, and every other operand shape
(comparison calls, bare commands, `not`) already carries an equivalent location this port checks
instead.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "always" => Style::Always,
            _ => Style::Conditionals,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::AndNode | NodeKind::OrNode => {
                if self.style == Style::Always {
                    let parent_kind = ctx.parent().map(|p| p.kind);
                    process_logical_operator(node, parent_kind, ctx);
                }
            }
            NodeKind::IfNode | NodeKind::UnlessNode | NodeKind::WhileNode | NodeKind::UntilNode
                if self.style == Style::Conditionals =>
            {
                if let Some(condition) = condition_of(node) {
                    find_logical_operators(&condition, None, ctx);
                }
            }
            _ => {}
        }
    }
}

/// The `predicate()` of an `if`/`unless`/`while`/`until` node -- upstream's
/// `on_if`'s (aliased `on_while`/`on_while_post`/`on_until`/`on_until_post`)
/// `node.condition`.
fn condition_of<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    match node.kind() {
        NodeKind::IfNode => node.as_if_node().map(|n| n.predicate()),
        NodeKind::UnlessNode => node.as_unless_node().map(|n| n.predicate()),
        NodeKind::WhileNode => node.as_while_node().map(|n| n.predicate()),
        NodeKind::UntilNode => node.as_until_node().map(|n| n.predicate()),
        _ => None,
    }
}

/// RuboCop's `node.condition.each_node(*AST::Node::OPERATOR_KEYWORDS)`: every
/// `and`/`or` node in `node`'s subtree (`node` itself included), each
/// processed alongside its own immediate AST parent's kind. See the module
/// doc for why this cannot simply reuse [`Context::ancestors`].
fn find_logical_operators(node: &Node<'_>, parent_kind: Option<NodeKind>, ctx: &mut Context<'_>) {
    if matches!(node.kind(), NodeKind::AndNode | NodeKind::OrNode) {
        process_logical_operator(node, parent_kind, ctx);
    }
    let this_kind = node.kind();
    for_each_child(node, |child| find_logical_operators(child, Some(this_kind), ctx));
}

/// RuboCop's `process_logical_operator`: skips a node already spelled
/// `&&`/`||` (`logical_operator?`), then reports the keyword spelling with a
/// fix that swaps the operator, wraps every operand that needs it to keep
/// its original meaning, and (`keep_operator_precedence`) wraps whichever
/// side of a mixed `and`/`or` chain needs it to preserve the original
/// grouping.
fn process_logical_operator(node: &Node<'_>, parent_kind: Option<NodeKind>, ctx: &mut Context<'_>) {
    let (left, right, operator_loc, alternate): (Node<'_>, Node<'_>, Location<'_>, &str) =
        match node.kind() {
            NodeKind::AndNode => {
                let Some(and) = node.as_and_node() else { return };
                (and.left(), and.right(), and.operator_loc(), "&&")
            }
            NodeKind::OrNode => {
                let Some(or) = node.as_or_node() else { return };
                (or.left(), or.right(), or.operator_loc(), "||")
            }
            _ => return,
        };

    let operator_text = ctx.text(operator_loc.span());
    if operator_text == b"&&" || operator_text == b"||" {
        return; // logical_operator?
    }
    let current = String::from_utf8_lossy(operator_text);
    let message = format!("Use `{alternate}` instead of `{current}`.");

    let mut edits = Vec::new();
    correct_child(&left, false, ctx, &mut edits);
    correct_child(&right, true, ctx, &mut edits);
    edits.push(Edit::replace(operator_loc.span(), alternate.as_bytes().to_vec()));

    if node.kind() == NodeKind::OrNode && parent_kind == Some(NodeKind::AndNode) {
        wrap(node.span(), &mut edits);
    } else if node.kind() == NodeKind::AndNode && right.kind() == NodeKind::OrNode {
        wrap(right.span(), &mut edits);
    }

    ctx.report_with_fix(
        &AndOr::META,
        operator_loc.span(),
        message,
        Fix { applicability: Applicability::Unsafe, edits },
    );
}

/// RuboCop's `node.each_child_node do |expr| ... end` body: dispatches one
/// operand of the `and`/`or` node to the correction it needs, or leaves it
/// untouched (a bare variable/literal never needs wrapping).
fn correct_child(expr: &Node<'_>, is_right: bool, ctx: &Context<'_>, edits: &mut Vec<Edit>) {
    if let Some(call) = expr.as_call_node() {
        correct_send(&call, is_right, ctx, edits);
        return;
    }
    let is_keyword_stmt = matches!(
        expr.kind(),
        NodeKind::ReturnNode | NodeKind::NextNode | NodeKind::BreakNode | NodeKind::YieldNode
    );
    if is_keyword_stmt || is_assignment_kind(expr.kind()) {
        correct_other(expr, is_right, edits);
    }
}

/// RuboCop's `correct_send`.
fn correct_send(call: &CallNode<'_>, is_right: bool, ctx: &Context<'_>, edits: &mut Vec<Edit>) {
    let name = call.name();
    let name = name.as_slice();

    if name == b"!" {
        correct_not(call, is_right, ctx, edits);
        return;
    }
    if call.equal_loc().is_some() {
        // setter_method?
        correct_setter(call, edits);
        return;
    }
    if is_comparison_method(name) {
        if call.opening_loc().is_none() {
            wrap(call.location().span(), edits);
        }
        return;
    }
    if !is_correctable_send(call) {
        return;
    }

    let Some(message_loc) = call.message_loc() else { return };
    let Some(arguments) = call.arguments() else { return };
    let Some(last_argument) = arguments.arguments().last() else { return };

    let begin = message_loc.span().end;
    let call_text = ctx.text(call.location().span());
    let end = if has_question_mark_touching_arg(call_text) { begin } else { begin + 1 };
    edits.push(Edit::replace(Span::new(begin, end), b"(".to_vec()));
    edits.push(Edit::insert(last_argument.span().end, b")".to_vec()));
}

/// RuboCop's `correct_not`: `!`/`not` is a special case, since both spell
/// method name `:!` -- `prefix_bang?` recurses into the negated call's own
/// correction, `prefix_not?` wraps the whole `not x` expression.
fn correct_not(call: &CallNode<'_>, is_right: bool, ctx: &Context<'_>, edits: &mut Vec<Edit>) {
    let Some(message_loc) = call.message_loc() else { return };
    match ctx.text(message_loc.span()) {
        b"!" => {
            let Some(receiver) = call.receiver() else { return };
            let Some(receiver_call) = receiver.as_call_node() else { return };
            correct_send(&receiver_call, is_right, ctx, edits);
        }
        b"not" if call.opening_loc().is_none() => {
            wrap(call.location().span(), edits);
        }
        _ => {}
    }
}

/// RuboCop's `correct_setter`: wraps from just before the receiver through
/// just after the last argument, e.g. `obj.attr = val` -> `(obj.attr = val)`.
fn correct_setter(call: &CallNode<'_>, edits: &mut Vec<Edit>) {
    let Some(receiver) = call.receiver() else { return };
    let Some(arguments) = call.arguments() else { return };
    let Some(last_argument) = arguments.arguments().last() else { return };
    edits.push(Edit::insert(receiver.span().start, b"(".to_vec()));
    edits.push(Edit::insert(last_argument.span().end, b")".to_vec()));
}

/// RuboCop's `correct_other`: wraps `node` in parentheses unless it is
/// already parenthesized, or is a bare (argument-less) `return`/`next`/
/// `break`/`yield` sitting on the right of the `and`/`or` node (where no
/// parentheses are needed since it already reads to the end of the
/// expression).
fn correct_other(node: &Node<'_>, is_right: bool, edits: &mut Vec<Edit>) {
    if is_already_parenthesized(node) {
        return;
    }
    if is_right && is_bare_keyword_stmt(node) {
        return;
    }
    wrap(node.span(), edits);
}

fn wrap(span: Span, edits: &mut Vec<Edit>) {
    edits.push(Edit::insert(span.start, b"(".to_vec()));
    edits.push(Edit::insert(span.end, b")".to_vec()));
}

/// RuboCop's `correctable_send?`.
fn is_correctable_send(call: &CallNode<'_>) -> bool {
    call.opening_loc().is_none()
        && call.arguments().is_some_and(|a| !a.arguments().is_empty())
        && call.name().as_slice() != b"[]"
}

/// RuboCop-AST's `Node::COMPARISON_OPERATORS`.
fn is_comparison_method(name: &[u8]) -> bool {
    matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=" | b">" | b"<")
}

/// RuboCop's `node.parenthesized_call?` for a `return`/`next`/`break`/
/// `yield` operand. See the `blind_spots` note on why only `yield` is
/// checked precisely.
fn is_already_parenthesized(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::YieldNode => node.as_yield_node().is_some_and(|y| y.lparen_loc().is_some()),
        _ => false,
    }
}

/// RuboCop's `node.children.empty?` for a `return`/`next`/`break`/`yield`
/// operand: true when it carries no arguments.
fn is_bare_keyword_stmt(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::ReturnNode => node.as_return_node().is_some_and(|n| n.arguments().is_none()),
        NodeKind::NextNode => node.as_next_node().is_some_and(|n| n.arguments().is_none()),
        NodeKind::BreakNode => node.as_break_node().is_some_and(|n| n.arguments().is_none()),
        NodeKind::YieldNode => node.as_yield_node().is_some_and(|n| n.arguments().is_none()),
        _ => false,
    }
}

/// RuboCop-AST's `Node::ASSIGNMENTS` (`EQUALS_ASSIGNMENTS` +
/// `SHORTHAND_ASSIGNMENTS`): every `lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`/
/// `casgn`/`masgn`/`op_asgn`/`or_asgn`/`and_asgn` shape, mapped onto Prism's
/// per-target-kind `*WriteNode` family (a plain `send`/`csend` setter is
/// handled separately by `correct_send`/`correct_setter`, matching upstream:
/// `send`/`csend` are not in `ASSIGNMENTS`).
fn is_assignment_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::MultiWriteNode
            | NodeKind::CallOperatorWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::CallOrWriteNode
            | NodeKind::IndexOperatorWriteNode
            | NodeKind::IndexAndWriteNode
            | NodeKind::IndexOrWriteNode
    )
}

/// RuboCop's `whitespace_before_arg`'s `/\?\S/.match?(node.source)` guard:
/// true when the call's own source has a `?` immediately touching a
/// non-whitespace character anywhere in it (a predicate method name written
/// with no space before its argument).
fn has_question_mark_touching_arg(text: &[u8]) -> bool {
    text.windows(2).any(|w| w[0] == b'?' && !w[1].is_ascii_whitespace())
}
