//! `Rails/Presence`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/presence.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `Base#MSG`.
fn message(prefer: &str, current: &str) -> String {
    format!("Use `{prefer}` instead of `{current}`.")
}

/// `OPERATOR_METHODS` of rubocop-ast's `MethodIdentifierPredicates`.
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

/// `ARITHMETIC_OPERATORS` of rubocop-ast's `MethodDispatchNode`.
const ARITHMETIC_OPERATORS: &[&[u8]] = &[b"+", b"-", b"*", b"/", b"%", b"**"];

/// Checks code that can be written more easily using `Object#presence`
/// defined by Active Support.
#[derive(Debug, Clone)]
pub struct Presence {
    /// For each enclosing call, whether an `if` that is its receiver or
    /// argument needs parentheses around the replacement: the call is a
    /// plain `send` (not `&.`) without parentheses of its own.
    calls: Vec<bool>,
}

impl Rule for Presence {
    const META: RuleMeta = RuleMeta {
        name: "Rails/Presence",
        department: Department::Rails,
        summary: "Checks code that can be written more easily using `Object#presence` defined \
                  by Active Support.",
        explanation: "Checks code that can be written more easily using `Object#presence` \
                      defined by Active Support.\n\n```ruby\n# bad\na.present? ? a : nil\n\n# \
                      bad\n!a.present? ? nil : a\n\n# bad\na.blank? ? nil : a\n\n# bad\n\
                      !a.blank? ? a : nil\n\n# good\na.presence\n```\n\n```ruby\n# bad\n\
                      a.present? ? a : b\n\n# bad\n!a.present? ? b : a\n\n# bad\na.blank? ? b : \
                      a\n\n# bad\n!a.blank? ? a : b\n\n# good\na.presence || b\n```\n\n\
                      ```ruby\n# bad\na.present? ? a.foo : nil\n\n# bad\n!a.present? ? nil : \
                      a.foo\n\n# bad\na.blank? ? nil : a.foo\n\n# bad\n!a.blank? ? a.foo : \
                      nil\n\n# good\na.presence&.foo\n```\n\n```ruby\n# good\na.present? ? \
                      a[1] : nil\n\n# good\na[:key] = value if a.present?\n\n# good\n\
                      a.present? ? a > 1 : nil\n\n# good\na <= 0 if a.present?\n\n# good\n\
                      a << \"bar\" if a.present?\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { calls: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                let Some(call) = node.as_call_node() else { return };
                let parenthesized =
                    call.opening_loc().is_some_and(|open| ctx.text(open.span()) == b"(");
                self.calls.push(!call.is_safe_navigation() && !parenthesized);
            }
            NodeKind::IfNode => {
                let Some(if_node) = node.as_if_node() else { return };
                let keyword = if_node.if_keyword_loc().map(|loc| loc.span());
                // `node.elsif?`.
                if keyword.is_some_and(|k| ctx.text(k) == b"elsif") {
                    return;
                }
                let if_branch = branch_of(if_node.statements());
                let else_branch = match if_node.subsequent() {
                    None => Branch::Missing,
                    Some(subsequent) => match subsequent.as_else_node() {
                        Some(else_node) => branch_of(else_node.statements()),
                        None => Branch::One(subsequent),
                    },
                };
                let parts = Parts {
                    node: node.span(),
                    condition: if_node.predicate(),
                    if_branch,
                    else_branch,
                    keyword,
                };
                self.check(&parts, ctx);
            }
            NodeKind::UnlessNode => {
                let Some(unless) = node.as_unless_node() else { return };
                // Whitequark builds `unless` as an `if` with the branches swapped.
                let if_branch = unless
                    .else_clause()
                    .map_or(Branch::Missing, |else_node| branch_of(else_node.statements()));
                let parts = Parts {
                    node: node.span(),
                    condition: unless.predicate(),
                    if_branch,
                    else_branch: branch_of(unless.statements()),
                    keyword: Some(unless.keyword_loc().span()),
                };
                self.check(&parts, ctx);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.kind() == NodeKind::CallNode {
            self.calls.pop();
        }
    }
}

/// An `if`'s branch as whitequark sees it: no statements is `nil`, one is
/// that statement, several are a `begin`.
enum Branch<'pr> {
    Missing,
    One(Node<'pr>),
    Many,
}

fn branch_of(statements: Option<ruby_ast::node::StatementsNode<'_>>) -> Branch<'_> {
    let Some(statements) = statements else { return Branch::Missing };
    let body = statements.body();
    let mut nodes = body.iter();
    match (nodes.next(), nodes.next()) {
        (None, _) => Branch::Missing,
        (Some(node), None) => Branch::One(node),
        (Some(_), Some(_)) => Branch::Many,
    }
}

struct Parts<'pr> {
    node: Span,
    condition: Node<'pr>,
    if_branch: Branch<'pr>,
    else_branch: Branch<'pr>,
    /// The `if`/`unless` keyword; `None` for a ternary.
    keyword: Option<Span>,
}

impl Presence {
    fn check(&self, parts: &Parts<'_>, ctx: &mut Context<'_>) {
        let Some((blank, receiver)) = condition_of(&parts.condition) else { return };
        // `receiver.nil?`: a bare `blank?`/`present?` ends the search just as
        // an ignored `other` does.
        let Some(receiver) = receiver else { return };

        // `redundant_receiver_and_other`: the branch that is the receiver
        // itself, and the other one, which must not be a `begin`.
        let (kept, other) = if blank {
            (&parts.else_branch, &parts.if_branch)
        } else {
            (&parts.if_branch, &parts.else_branch)
        };
        let other_node = match other {
            Branch::Missing => Some(None),
            Branch::One(node) if node.kind() != NodeKind::ParenthesesNode => Some(Some(node)),
            _ => None,
        };
        if let (Some(other_node), Branch::One(kept_node)) = (other_node, kept) {
            if same_node(ctx, kept_node, &receiver) {
                if other_node.is_some_and(is_ignored_other) {
                    return;
                }
                self.register_offense(parts, ctx, &receiver, other_node);
                return;
            }
        }

        // `redundant_receiver_and_chain`.
        let (chain, empty) = if blank {
            (&parts.else_branch, &parts.if_branch)
        } else {
            (&parts.if_branch, &parts.else_branch)
        };
        let empty_ok = match empty {
            Branch::Missing => true,
            Branch::One(node) => node.kind() == NodeKind::NilNode,
            Branch::Many => false,
        };
        let Branch::One(chain) = chain else { return };
        if !empty_ok {
            return;
        }
        let Some(call) = as_send(chain) else { return };
        if !call.receiver().is_some_and(|r| same_node(ctx, &r, &receiver)) {
            return;
        }
        // `ignore_chain_node?`: `assignment?` of a `send` is `setter_method?`.
        if call.is_attribute_write() || OPERATOR_METHODS.contains(&call.name().as_slice()) {
            return;
        }
        let mut replacement = format!(
            "{}.presence&.{}",
            source(ctx, &receiver),
            String::from_utf8_lossy(call.name().as_slice())
        );
        if has_arguments(&call) {
            replacement.push('(');
            replacement.push_str(&argument_sources(ctx, &call).join(", "));
            replacement.push(')');
        }
        Self::report(parts, ctx, &replacement);
    }

    fn register_offense(
        &self,
        parts: &Parts<'_>,
        ctx: &mut Context<'_>,
        receiver: &Node<'_>,
        other: Option<&Node<'_>>,
    ) {
        let or_source = match other {
            None => String::new(),
            Some(other) => match as_send(other) {
                Some(call) => build_source_for_or_method(ctx, &call, other),
                None if other.kind() == NodeKind::NilNode => String::new(),
                None => format!(" || {}", source(ctx, other)),
            },
        };
        let mut replacement = format!("{}.presence{or_source}", source(ctx, receiver));
        if self.require_parentheses(ctx, &or_source) {
            replacement = format!("({replacement})");
        }
        Self::report(parts, ctx, &replacement);
    }

    /// `require_parentheses?`: the `if` is an operand of an unparenthesized
    /// `send`, or of an `and`.
    fn require_parentheses(&self, ctx: &Context<'_>, or_source: &str) -> bool {
        if or_source.is_empty() {
            return false;
        }
        let ancestors = ctx.ancestors();
        let parent = ancestors.last().map(|a| a.kind);
        if parent == Some(NodeKind::AndNode) {
            return true;
        }
        let call_parent = match parent {
            Some(NodeKind::CallNode) => true,
            Some(NodeKind::ArgumentsNode) => {
                ancestors.len() >= 2 && ancestors[ancestors.len() - 2].kind == NodeKind::CallNode
            }
            _ => false,
        };
        call_parent && self.calls.last().copied().unwrap_or(false)
    }

    fn report(parts: &Parts<'_>, ctx: &mut Context<'_>, replacement: &str) {
        let message = message(&squash(replacement), &squash(&current(ctx, parts)));
        ctx.report_with_fix(
            &Self::META,
            parts.node,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(parts.node, replacement.as_bytes().to_vec())],
            },
        );
    }
}

/// `current(node)`.
fn current(ctx: &Context<'_>, parts: &Parts<'_>) -> String {
    let text = String::from_utf8_lossy(ctx.text(parts.node)).into_owned();
    if let (Some(keyword), true) = (parts.keyword, text.contains('\n')) {
        let selector_end = parts
            .condition
            .as_call_node()
            .and_then(|call| call.message_loc())
            .map_or(keyword.end, |loc| loc.span().end);
        let head = String::from_utf8_lossy(ctx.text(Span::new(keyword.start, selector_end)));
        return format!("{head} ... end");
    }
    // `gsub(/\n\s*/, ' ')`.
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            while chars.next_if(|c| c.is_whitespace()).is_some() {}
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// `gsub(/^\s*|\n/, '')`: drops newlines and every line's leading whitespace.
fn squash(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at_line_start = true;
    for c in text.chars() {
        if c == '\n' {
            at_line_start = true;
        } else if !(at_line_start && c.is_whitespace()) {
            out.push(c);
            at_line_start = false;
        }
    }
    out
}

/// The receiver of a plain call; a distinct type so a missing receiver is not
/// confused with a non-matching call.
struct Receiver<'pr>(Option<Node<'pr>>);

/// `(send ...)`: a call that is neither `&.` nor given a literal block.
fn as_send<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    let literal_block = call.block().is_some_and(|b| b.as_block_argument_node().is_none());
    (!call.is_safe_navigation() && !literal_block).then_some(call)
}

/// `(send $_recv :name)` without arguments: the receiver.
fn plain_call<'pr>(node: &Node<'pr>, name: &[u8]) -> Option<Receiver<'pr>> {
    let call = as_send(node)?;
    (call.name().as_slice() == name && !has_arguments(&call)).then(|| Receiver(call.receiver()))
}

/// `{(send $_recv :blank?) (send (send $_recv :present?) :!)}` (`true`) or
/// the `present?` counterpart (`false`), with the captured receiver.
fn condition_of<'pr>(condition: &Node<'pr>) -> Option<(bool, Option<Node<'pr>>)> {
    let negated = |inner: &[u8]| {
        let outer = as_send(condition)?;
        if outer.name().as_slice() != b"!" || has_arguments(&outer) {
            return None;
        }
        plain_call(&outer.receiver()?, inner)
    };
    if let Some(receiver) = plain_call(condition, b"blank?").or_else(|| negated(b"present?")) {
        return Some((true, receiver.0));
    }
    plain_call(condition, b"present?").or_else(|| negated(b"blank?")).map(|r| (false, r.0))
}

/// `ignore_other_node?`: `if`, `rescue` and `while` nodes.
fn is_ignored_other(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::IfNode
            | NodeKind::UnlessNode
            | NodeKind::RescueModifierNode
            | NodeKind::WhileNode
    )
}

/// `arguments?`: a `&block` is one more argument in whitequark.
fn has_arguments(call: &CallNode<'_>) -> bool {
    call.arguments().is_some() || call.block().is_some_and(|b| b.as_block_argument_node().is_some())
}

/// `arguments.map(&:source)`.
fn argument_sources(ctx: &Context<'_>, call: &CallNode<'_>) -> Vec<String> {
    let mut sources: Vec<String> = call
        .arguments()
        .map(|a| a.arguments().iter().map(|arg| source(ctx, &arg)).collect())
        .unwrap_or_default();
    if let Some(block) = call.block().filter(|b| b.as_block_argument_node().is_some()) {
        sources.push(source(ctx, &block));
    }
    sources
}

fn source(ctx: &Context<'_>, node: &Node<'_>) -> String {
    String::from_utf8_lossy(ctx.text(node.span())).into_owned()
}

/// `build_source_for_or_method`.
fn build_source_for_or_method(ctx: &Context<'_>, call: &CallNode<'_>, node: &Node<'_>) -> String {
    let name = call.name();
    let name = name.as_slice();
    let parenthesized = call.opening_loc().is_some_and(|open| ctx.text(open.span()) == b"(");
    if parenthesized
        || name == b"[]"
        || ARITHMETIC_OPERATORS.contains(&name)
        || !has_arguments(call)
    {
        return format!(" || {}", source(ctx, node));
    }
    let first_argument = call
        .arguments()
        .and_then(|a| a.arguments().iter().next().map(|arg| arg.span().start))
        .or_else(|| call.block().map(|b| b.span().start))
        .unwrap_or(node.span().end);
    // `range_between(begin_pos, first_argument.begin_pos - 1)`.
    let method = String::from_utf8_lossy(
        ctx.text(Span::new(node.span().start, first_argument.saturating_sub(1))),
    )
    .into_owned();
    format!(" || {method}({})", argument_sources(ctx, call).join(", "))
}

/// AST equality of two nodes, ignoring layout: the same kinds all the way
/// down, with the text between children (names, operators, literals)
/// equal modulo whitespace.
fn same_node(ctx: &Context<'_>, a: &Node<'_>, b: &Node<'_>) -> bool {
    if a.kind() != b.kind() || skeleton(ctx, a) != skeleton(ctx, b) {
        return false;
    }
    let (mut children_a, mut children_b) = (Vec::new(), Vec::new());
    collect_children(a, &mut children_a);
    collect_children(b, &mut children_b);
    children_a.len() == children_b.len()
        && children_a.iter().zip(&children_b).all(|(x, y)| same_node(ctx, x, y))
}

fn collect_children<'pr>(node: &Node<'pr>, out: &mut Vec<Node<'pr>>) {
    for_each_child(node, |child| out.push(*child));
}

/// The node's text with its children's text cut out and whitespace removed.
fn skeleton(ctx: &Context<'_>, node: &Node<'_>) -> Vec<u8> {
    let span = node.span();
    let mut children = Vec::new();
    for_each_child(node, |child| children.push(child.span()));
    let mut out = Vec::new();
    let mut at = span.start;
    for child in children {
        if child.start >= at && child.end <= span.end {
            out.extend_from_slice(ctx.text(Span::new(at, child.start)));
            at = child.end;
        }
    }
    out.extend_from_slice(ctx.text(Span::new(at, span.end)));
    out.retain(|byte| !byte.is_ascii_whitespace());
    out
}
