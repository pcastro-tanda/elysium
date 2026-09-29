//! `Style/NilComparison`, ported from RuboCop's
//! `lib/rubocop/cop/style/nil_comparison.rb`.
//!
//! # Wrapping the `EnforcedStyle: comparison` autocorrection
//!
//! `autocorrect_to_comparison`'s `operator_expression?(node.parent)` check
//! (used to decide whether the new ` == nil` needs wrapping in parens to
//! preserve precedence, e.g. `!x.nil?` -> `!(x == nil)`) needs the *real*
//! logical parent node, not just its `NodeKind`/`Span` (`Context::ancestors`
//! only exposes that much). [`logical_parent`] recovers the right ancestor
//! (bridged across the whitequark/Prism shape gaps -- an interposed
//! `ArgumentsNode`, and a single-statement `StatementsNode` -- exactly like
//! `Lint/RedundantSplatExpansion`'s identical helper, copied here private to
//! this file per the porting kit); [`find_node_by_span`] then walks down
//! from the parsed root along the single child chain containing that span
//! to recover the actual, accessor-bearing [`Node`].

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    NodeInfo, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `PREDICATE_MSG`.
const PREDICATE_MSG: &str = "Prefer the use of the `nil?` predicate.";
/// RuboCop's `EXPLICIT_MSG`.
const EXPLICIT_MSG: &str = "Prefer the use of the `==` comparison.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Predicate,
    Comparison,
}

/// Checks for comparison of something with nil using `==`/`===` and `nil?`.
#[derive(Debug, Clone)]
pub struct NilComparison {
    style: Style,
}

impl Rule for NilComparison {
    const META: RuleMeta = RuleMeta {
        name: "Style/NilComparison",
        department: Department::Style,
        summary: "Checks for comparison of something with nil using `==` and `nil?`.",
        explanation: "\
Checks for comparison of something with nil using `==` and `nil?`. \
Enforcing a consistent style (either the `nil?` predicate or `==` \
comparison) improves readability.

```ruby
# EnforcedStyle: predicate (default)

# bad
if x == nil
end

# good
if x.nil?
end
```

```ruby
# EnforcedStyle: comparison

# bad
if x.nil?
end

# good
if x == nil
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("predicate"),
            allowed: &["predicate", "comparison"],
            doc: "Whether to prefer the `nil?` predicate or `==`/`===` comparison against `nil`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "comparison" => Style::Comparison,
            _ => Style::Predicate,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.receiver().is_none() {
            return;
        }
        match self.style {
            Style::Predicate => check_nil_comparison(node, &call, ctx),
            Style::Comparison => check_nil_check(node, &call, ctx),
        }
    }
}

/// RuboCop's `nil_comparison?` matcher (`(send _ {:== :===} nil)`) plus
/// `on_send`'s `EnforcedStyle: predicate` branch.
fn check_nil_comparison(node: &Node<'_>, call: &CallNode<'_>, ctx: &mut Context<'_>) {
    let name = call.name();
    if !matches!(name.as_slice(), b"==" | b"===") || !is_sole_nil_argument(call) {
        return;
    }
    let Some(message_loc) = call.message_loc() else { return };
    let receiver = call.receiver().expect("receiver checked by caller");
    let fix = autocorrect_to_predicate(ctx, node.span(), &receiver);
    ctx.report_with_fix(&NilComparison::META, message_loc.span(), PREDICATE_MSG, fix);
}

/// RuboCop's `nil_check?` matcher (`(send _ :nil?)`) plus `on_send`'s
/// `EnforcedStyle: comparison` branch.
fn check_nil_check(node: &Node<'_>, call: &CallNode<'_>, ctx: &mut Context<'_>) {
    let name = call.name();
    if name.as_slice() != b"nil?" {
        return;
    }
    let Some(message_loc) = call.message_loc() else { return };
    let fix = autocorrect_to_comparison(ctx, node.span(), call);
    ctx.report_with_fix(&NilComparison::META, message_loc.span(), EXPLICIT_MSG, fix);
}

/// RuboCop's `nil_comparison?` argument shape: exactly one argument, a
/// literal `nil`.
fn is_sole_nil_argument(call: &CallNode<'_>) -> bool {
    let Some(args) = call.arguments() else { return false };
    let items: Vec<Node<'_>> = args.arguments().iter().collect();
    matches!(items.as_slice(), [only] if only.kind() == NodeKind::NilNode)
}

/// RuboCop's `autocorrect_to_predicate`.
fn autocorrect_to_predicate(ctx: &Context<'_>, call_span: Span, receiver: &Node<'_>) -> Fix {
    let edit = if operator_expression(receiver) {
        let source = String::from_utf8_lossy(ctx.text(receiver.span())).into_owned();
        Edit::replace(call_span, format!("({source}).nil?").into_bytes())
    } else {
        let range = Span::new(receiver.span().end, call_span.end);
        Edit::replace(range, b".nil?".to_vec())
    };
    Fix { applicability: Applicability::Safe, edits: vec![edit] }
}

/// RuboCop's `autocorrect_to_comparison`.
fn autocorrect_to_comparison(ctx: &Context<'_>, call_span: Span, call: &CallNode<'_>) -> Fix {
    let mut edits = Vec::new();
    if let (Some(dot), Some(message)) = (call.call_operator_loc(), call.message_loc()) {
        let range = Span::new(dot.span().start, message.span().end);
        edits.push(Edit::replace(range, b" == nil".to_vec()));
    }
    if wraps_in_parens(ctx, call_span) {
        edits.push(Edit::insert(call_span.start, b"(".to_vec()));
        edits.push(Edit::insert(call_span.end, b")".to_vec()));
    }
    Fix { applicability: Applicability::Safe, edits }
}

/// Whether `call_span`'s logical parent is an [`operator_expression`], so
/// the new ` == nil` comparison -- which binds looser than most operators --
/// needs wrapping in parentheses to preserve the original precedence.
fn wraps_in_parens(ctx: &Context<'_>, call_span: Span) -> bool {
    let Some(parent) = logical_parent(ctx.ancestors(), call_span) else { return false };
    let Some(parent_node) = find_node_by_span(ctx.parsed().root(), parent.span, parent.kind) else {
        return false;
    };
    operator_expression(&parent_node)
}

/// RuboCop's `operator_expression?`.
fn operator_expression(node: &Node<'_>) -> bool {
    if let Some(call) = node.as_call_node() {
        let name = call.name();
        if !call.is_safe_navigation() && is_operator_method(name.as_slice()) {
            return true;
        }
    }
    match node.kind() {
        NodeKind::AndNode | NodeKind::OrNode | NodeKind::RangeNode | NodeKind::FlipFlopNode => true,
        NodeKind::IfNode => {
            node.as_if_node().is_some_and(|if_node| if_node.if_keyword_loc().is_none())
        }
        kind => is_assignment_kind(kind),
    }
}

/// RuboCop's `operator_send?`: an `OPERATOR_METHODS` name (excluding `[]`
/// and `[]=`, which upstream carves out separately) on a non-safe-navigation
/// call.
fn is_operator_method(name: &[u8]) -> bool {
    matches!(
        name,
        b"|" | b"^"
            | b"&"
            | b"<=>"
            | b"=="
            | b"==="
            | b"=~"
            | b">"
            | b">="
            | b"<"
            | b"<="
            | b"<<"
            | b">>"
            | b"+"
            | b"-"
            | b"*"
            | b"/"
            | b"%"
            | b"**"
            | b"~"
            | b"+@"
            | b"-@"
            | b"!@"
            | b"~@"
            | b"!"
            | b"!="
            | b"!~"
            | b"`"
    )
}

/// RuboCop-AST's `Node::ASSIGNMENTS` (`EQUALS_ASSIGNMENTS` +
/// `SHORTHAND_ASSIGNMENTS`): every plain, compound, `||=` and `&&=`
/// assignment target Prism has a node kind for.
fn is_assignment_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::MultiWriteNode
            | NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::CallOperatorWriteNode
            | NodeKind::IndexOperatorWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::CallOrWriteNode
            | NodeKind::IndexOrWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::IndexAndWriteNode
    )
}

/// `node.parent`, bridged across two whitequark/Prism shape gaps: an
/// interposed `ArgumentsNode` (absent from whitequark, where a call's
/// arguments are its own direct children), and a `StatementsNode` that holds
/// only one statement (whitequark elides a single-statement `begin`, so its
/// "parent" skips straight past where Prism always wraps one). Once the
/// walk reaches the top (`ProgramNode`), whitequark has no equivalent node
/// at all, so that is reported as no parent (`None`). Copied from
/// `Lint/RedundantSplatExpansion`'s identical `logical_ancestor` (with
/// `levels` fixed at 1, since only the immediate parent is ever needed
/// here).
fn logical_parent(ancestors: &[NodeInfo], node_span: Span) -> Option<NodeInfo> {
    let mut current_span = node_span;
    let mut idx = ancestors.len();
    loop {
        idx = idx.checked_sub(1)?;
        let info = ancestors[idx];
        let transparent = info.kind == NodeKind::ArgumentsNode
            || (info.kind == NodeKind::StatementsNode && info.span == current_span);
        current_span = info.span;
        if transparent {
            continue;
        }
        return (info.kind != NodeKind::ProgramNode).then_some(info);
    }
}

/// Descends from `node` along the single child chain containing `span`,
/// returning the actual node of kind `kind` occupying it. A wrapper node
/// (`ProgramNode`/`StatementsNode`) can share its lone child's exact span,
/// so the kind must be checked too, not just the span, or the search would
/// stop one level too high.
fn find_node_by_span(node: Node<'_>, span: Span, kind: NodeKind) -> Option<Node<'_>> {
    if node.span() == span && node.kind() == kind {
        return Some(node);
    }
    let mut found = None;
    for_each_child(&node, |child| {
        if found.is_none() && child.span().start <= span.start && span.end <= child.span().end {
            found = find_node_by_span(*child, span, kind);
        }
    });
    found
}
