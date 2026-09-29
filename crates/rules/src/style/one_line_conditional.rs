//! `Style/OneLineConditional`, ported from RuboCop's
//! `lib/rubocop/cop/style/one_line_conditional.rb` plus its
//! `IfThenCorrector`.
//!
//! # Gate order
//!
//! Mirrors `OnNormalIfUnless#on_normal_if_unless` exactly: single-line
//! first, then a genuine `else` present, then not an `elsif` link itself
//! and not a multi-statement `then` body. "Genuine `else` present" reads
//! [`IfNode::subsequent`]/[`UnlessNode::else_clause`]: an `elsif` link
//! (another [`IfNode`] reached through `subsequent`) always counts, no
//! matter what follows it (even nothing); a real [`ElseNode`] only counts
//! when its own `statements` is `Some` -- an empty `else` clause reads as
//! "no else" both here and, recursively, inside [`build_if`]'s own
//! `subsequent` walk (matching whitequark's nil-for-empty-body shape,
//! which upstream's `else_branch`/`rewrite_else_branch` rely on).
//!
//! # `ignore_node`
//!
//! A one-line `if`/`unless` nested inside another one's branch is visited
//! independently (it is a real, separate [`IfNode`]/[`UnlessNode`]) and
//! gets its own offense -- but only the *first* (outermost, since the
//! engine's walk is pre-order) actually autocorrects; `self.ignored`
//! mirrors `part_of_ignored_node?`/`ignore_node`, recording the span of
//! every node this rule has already rewritten so a later, nested offense
//! whose span falls inside one is reported without a fix.
//!
//! # The `~`/`!` "unary-looking infix" trap
//!
//! `a ~ if cond then x else y end` and `a ! if cond then x else y end`
//! parse (confirmed against Prism directly) not as an infix `~`/`!` call
//! on `a`, but as `a(~(if...end))`: a call to `a` with one argument, a
//! *unary* `~`/`!` send whose *receiver* is the `if` node. Every other
//! tested operator (`+`, `<<`, `==`, ...) instead parses as the ordinary
//! infix shape, `CallNode { receiver: a, name: OP, arguments: [if...end] }`
//! -- the `if` node sits inside an `ArgumentsNode`, one level further from
//! its own immediate parent than the `~`/`!` case. [`needs_wrap`] handles
//! both shapes uniformly: it does not attempt to read the ambient call's
//! own `name()` (unavailable from [`linter::Context::ancestors`], which
//! only carries kind and span for anything above the immediate node), and
//! instead reads back the literal operator token as plain source text
//! between whichever ancestor's span starts the search and the node's own
//! span start.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, NodeInfo, OptionError,
    OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, IfNode, UnlessNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop-AST's `OPERATOR_METHODS`.
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

/// RuboCop-AST's `Node::ASSIGNMENTS` (`EQUALS_ASSIGNMENTS` +
/// `SHORTHAND_ASSIGNMENTS`): Prism splits each assignment shape into a
/// per-target `*WriteNode` kind where whitequark uses one generic node
/// type, so this lists every one of them regardless of the target being a
/// variable, constant, method call, or index.
fn is_assignment(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::MultiWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::IndexAndWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::CallOrWriteNode
            | NodeKind::IndexOrWriteNode
            | NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::CallOperatorWriteNode
            | NodeKind::IndexOperatorWriteNode
    )
}

/// RuboCop's `prefix_not?`: a `!` call with an explicit receiver whose own
/// selector reads `not` rather than `!`.
fn is_prefix_not(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    call.name().as_slice() == b"!"
        && call.receiver().is_some()
        && call.message_loc().is_some_and(|l| ctx.text(l.span()) == b"not")
}

/// RuboCop's `requires_parentheses?` plus `method_call_with_changed_precedence?`
/// and `keyword_with_changed_precedence?`.
fn requires_parentheses(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    match node.kind() {
        NodeKind::AndNode | NodeKind::OrNode | NodeKind::IfNode | NodeKind::UnlessNode => true,
        kind if is_assignment(kind) => true,
        NodeKind::CallNode => {
            let call = node.as_call_node().expect("kind matched");
            if is_prefix_not(&call, ctx) {
                return true;
            }
            if call.is_safe_navigation() {
                return false;
            }
            let has_args = call.arguments().is_some_and(|a| !a.arguments().is_empty());
            has_args
                && call.opening_loc().is_none()
                && !OPERATOR_METHODS.contains(&call.name().as_slice())
        }
        NodeKind::DefinedNode => {
            node.as_defined_node().expect("kind matched").lparen_loc().is_none()
        }
        NodeKind::YieldNode => {
            let y = node.as_yield_node().expect("kind matched");
            y.lparen_loc().is_none() && y.arguments().is_some_and(|a| !a.arguments().is_empty())
        }
        NodeKind::SuperNode => {
            let s = node.as_super_node().expect("kind matched");
            s.lparen_loc().is_none() && s.arguments().is_some_and(|a| !a.arguments().is_empty())
        }
        NodeKind::BreakNode => node
            .as_break_node()
            .expect("kind matched")
            .arguments()
            .is_some_and(|a| !a.arguments().is_empty()),
        NodeKind::NextNode => node
            .as_next_node()
            .expect("kind matched")
            .arguments()
            .is_some_and(|a| !a.arguments().is_empty()),
        NodeKind::ReturnNode => node
            .as_return_node()
            .expect("kind matched")
            .arguments()
            .is_some_and(|a| !a.arguments().is_empty()),
        _ => false,
    }
}

/// RuboCop's `expr_replacement`.
fn expr_text(node: Option<&Node<'_>>, ctx: &Context<'_>) -> Vec<u8> {
    let Some(node) = node else { return b"nil".to_vec() };
    let text = ctx.text(node.span());
    if requires_parentheses(node, ctx) {
        let mut out = Vec::with_capacity(text.len() + 2);
        out.push(b'(');
        out.extend_from_slice(text);
        out.push(b')');
        out
    } else {
        text.to_vec()
    }
}

/// The literal token of source text between `[start, end)`, trimmed of
/// surrounding whitespace and narrowed to its trailing whitespace-delimited
/// word -- the operator selector for both the infix shape
/// (`CallNode { receiver, name: OP, arguments: [node] }`, where `start` is
/// the call's own span start, at the receiver) and the `~`/`!`
/// "unary-looking" shape (`CallNode { receiver: node, name: "~" }`, where
/// `start` is the call's own span start, at the operator itself).
fn trailing_operator(ctx: &Context<'_>, start: u32, end: u32) -> bool {
    if end <= start {
        return false;
    }
    let text = ctx.text(Span::new(start, end));
    let trimmed_end = text.iter().rposition(|b| !b.is_ascii_whitespace()).map_or(0, |i| i + 1);
    let trimmed = &text[..trimmed_end];
    let word_start = trimmed.iter().rposition(u8::is_ascii_whitespace).map_or(0, |i| i + 1);
    OPERATOR_METHODS.contains(&&trimmed[word_start..])
}

/// RuboCop's `node.parent.operator_keyword?` / `node.parent.send_type? &&
/// node.parent.operator_method?`, deciding whether the ternary replacement
/// needs an extra pair of wrapping parens. See the module doc for the
/// `~`/`!` shape this also has to cover.
fn needs_wrap(ancestors: &[NodeInfo], ctx: &Context<'_>, node_start: u32) -> bool {
    let Some(&parent) = ancestors.last() else { return false };
    match parent.kind {
        NodeKind::AndNode | NodeKind::OrNode => true,
        NodeKind::CallNode => trailing_operator(ctx, parent.span.start, node_start),
        NodeKind::ArgumentsNode => {
            ancestors.len() >= 2 && {
                let grand = ancestors[ancestors.len() - 2];
                grand.kind == NodeKind::CallNode
                    && trailing_operator(ctx, grand.span.start, node_start)
            }
        }
        _ => false,
    }
}

/// `node.subsequent`/`node.else_clause`'s "genuine else" gate: an `elsif`
/// link always counts; a real `else` clause only counts when it has a
/// body (an empty one reads as whitequark's nil `else_branch`, i.e. "no
/// else at all").
fn has_genuine_else(subsequent: Option<Node<'_>>) -> bool {
    match subsequent {
        None => false,
        Some(node) => match node.as_if_node() {
            Some(_) => true,
            None => node.as_else_node().is_some_and(|e| e.statements().is_some()),
        },
    }
}

/// RuboCop's `cannot_replace_to_ternary?` for `if`/`elsif`: an `elsif`
/// chain can never fit a ternary, and neither can a real `else` whose body
/// holds two or more statements.
fn cannot_replace_to_ternary_if(node: &IfNode<'_>) -> bool {
    let Some(subsequent) = node.subsequent() else { return false };
    if subsequent.as_if_node().is_some() {
        return true;
    }
    subsequent.as_else_node().and_then(|e| e.statements()).is_some_and(|s| s.body().len() >= 2)
}

/// RuboCop's `cannot_replace_to_ternary?` for `unless` (no `elsif` chain is
/// possible).
fn cannot_replace_to_ternary_unless(node: &UnlessNode<'_>) -> bool {
    node.else_clause().and_then(|e| e.statements()).is_some_and(|s| s.body().len() >= 2)
}

/// RuboCop's `MSG_TERNARY`/`MSG_MULTILINE`.
fn message(keyword: &str, multiline: bool) -> String {
    if multiline {
        format!(
            "Favor multi-line `{keyword}` over single-line `{keyword}/then/else/end` constructs."
        )
    } else {
        format!(
            "Favor the ternary operator (`?:`) over single-line `{keyword}/then/else/end` constructs."
        )
    }
}

fn spaces(n: u32) -> Vec<u8> {
    vec![b' '; n as usize]
}

/// RuboCop's `IfThenCorrector#replacement` for an `if`/`elsif` chain:
/// `base_indent` (the head node's own column, shared unchanged through
/// every recursive `elsif`/`else` link) and `body_indent` (this cop's
/// `configured_indentation_width`) never change across the recursion --
/// only the recursion's own keyword (`if`/`elsif`) and condition/branch
/// text do.
fn build_if(
    ctx: &Context<'_>,
    node: &IfNode<'_>,
    base_indent: &[u8],
    body_indent: &[u8],
) -> Vec<u8> {
    let is_elsif = node.if_keyword_loc().is_some_and(|k| ctx.text(k.span()) == b"elsif");
    let mut out = Vec::new();
    if is_elsif {
        out.extend_from_slice(base_indent);
        out.extend_from_slice(b"elsif ");
    } else {
        out.extend_from_slice(b"if ");
    }
    out.extend_from_slice(ctx.text(node.predicate().span()));
    out.push(b'\n');
    out.extend_from_slice(base_indent);
    out.extend_from_slice(body_indent);
    match node.statements() {
        Some(s) => out.extend_from_slice(ctx.text(s.as_node().span())),
        None => out.extend_from_slice(b"nil"),
    }
    out.push(b'\n');
    match node.subsequent() {
        None => out.extend_from_slice(b"end"),
        Some(sub) => {
            if let Some(next) = sub.as_if_node() {
                out.extend_from_slice(&build_if(ctx, &next, base_indent, body_indent));
            } else {
                let else_node = sub.as_else_node().expect("subsequent is either if or else");
                match else_node.statements() {
                    Some(s) => {
                        out.extend_from_slice(base_indent);
                        out.extend_from_slice(b"else\n");
                        out.extend_from_slice(base_indent);
                        out.extend_from_slice(body_indent);
                        out.extend_from_slice(ctx.text(s.as_node().span()));
                        out.push(b'\n');
                        out.extend_from_slice(base_indent);
                        out.extend_from_slice(b"end");
                    }
                    // Matches whitequark's nil `else_branch` for an empty
                    // `else`: treated identically to no `else` at all.
                    None => out.extend_from_slice(b"end"),
                }
            }
        }
    }
    out
}

/// RuboCop's `IfThenCorrector#replacement` for `unless` (no `elsif` chain).
fn build_unless(
    ctx: &Context<'_>,
    node: &UnlessNode<'_>,
    base_indent: &[u8],
    body_indent: &[u8],
) -> Vec<u8> {
    let mut out = b"unless ".to_vec();
    out.extend_from_slice(ctx.text(node.predicate().span()));
    out.push(b'\n');
    out.extend_from_slice(base_indent);
    out.extend_from_slice(body_indent);
    match node.statements() {
        Some(s) => out.extend_from_slice(ctx.text(s.as_node().span())),
        None => out.extend_from_slice(b"nil"),
    }
    out.push(b'\n');
    out.extend_from_slice(base_indent);
    match node.else_clause().and_then(|e| e.statements()) {
        Some(s) => {
            out.extend_from_slice(b"else\n");
            out.extend_from_slice(base_indent);
            out.extend_from_slice(body_indent);
            out.extend_from_slice(ctx.text(s.as_node().span()));
            out.push(b'\n');
            out.extend_from_slice(base_indent);
            out.extend_from_slice(b"end");
        }
        None => out.extend_from_slice(b"end"),
    }
    out
}

/// Favor the ternary operator (?:) or multi-line constructs over single-line if/then/else/end constructs.
#[derive(Debug, Clone)]
pub struct OneLineConditional {
    /// `AlwaysCorrectToMultiline`.
    always_multiline: bool,
    /// `Alignment#configured_indentation_width`: `Layout/IndentationWidth`'s
    /// `Width`, else `2`.
    indentation_width: u32,
    /// RuboCop's `ignore_node`/`part_of_ignored_node?`: spans already
    /// autocorrected this file, so a one-line conditional nested inside one
    /// of them is reported without a fix.
    ignored: Vec<Span>,
}

impl OneLineConditional {
    fn contained_in_ignored(&self, span: Span) -> bool {
        self.ignored.iter().any(|s| s.start <= span.start && span.end <= s.end)
    }

    fn ternary_replacement(
        ctx: &Context<'_>,
        span: Span,
        predicate: &Node<'_>,
        first: Option<Node<'_>>,
        second: Option<Node<'_>>,
    ) -> Vec<u8> {
        let mut inner = expr_text(Some(predicate), ctx);
        inner.extend_from_slice(b" ? ");
        inner.extend_from_slice(&expr_text(first.as_ref(), ctx));
        inner.extend_from_slice(b" : ");
        inner.extend_from_slice(&expr_text(second.as_ref(), ctx));
        if needs_wrap(ctx.ancestors(), ctx, span.start) {
            let mut out = Vec::with_capacity(inner.len() + 2);
            out.push(b'(');
            out.append(&mut inner);
            out.push(b')');
            out
        } else {
            inner
        }
    }

    fn check_if(&mut self, node: &IfNode<'_>, ctx: &mut Context<'_>) {
        // Ternary (upstream's `on_if`: `return if node.ternary?`).
        let Some(if_kw) = node.if_keyword_loc() else { return };
        let span = node.as_node().span();
        if !ctx.is_single_line(span) {
            return;
        }
        if ctx.text(if_kw.span()) == b"elsif" {
            return;
        }
        if !has_genuine_else(node.subsequent()) {
            return;
        }
        if node.statements().is_some_and(|s| s.body().len() > 1) {
            return;
        }

        let multiline = self.always_multiline || cannot_replace_to_ternary_if(node);
        let msg = message("if", multiline);

        if self.contained_in_ignored(span) {
            ctx.report(&Self::META, span, msg);
            return;
        }

        let replacement = if multiline {
            let base_indent = spaces(ctx.line_col(span.start).column);
            let body_indent = spaces(self.indentation_width);
            build_if(ctx, node, &base_indent, &body_indent)
        } else {
            let predicate = node.predicate();
            let if_branch = node.statements().and_then(|s| s.body().first());
            let else_node = node
                .subsequent()
                .and_then(|s| s.as_else_node())
                .expect("ternary path implies a plain else, not an elsif chain");
            let else_branch = else_node
                .statements()
                .and_then(|s| s.body().first())
                .expect("guarded: exactly one statement");
            Self::ternary_replacement(ctx, span, &predicate, if_branch, Some(else_branch))
        };

        ctx.report_with_fix(
            &Self::META,
            span,
            msg,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement)],
            },
        );
        self.ignored.push(span);
    }

    fn check_unless(&mut self, node: &UnlessNode<'_>, ctx: &mut Context<'_>) {
        let span = node.as_node().span();
        if !ctx.is_single_line(span) {
            return;
        }
        let Some(else_clause) = node.else_clause() else { return };
        if else_clause.statements().is_none() {
            return;
        }
        if node.statements().is_some_and(|s| s.body().len() > 1) {
            return;
        }

        let multiline = self.always_multiline || cannot_replace_to_ternary_unless(node);
        let msg = message("unless", multiline);

        if self.contained_in_ignored(span) {
            ctx.report(&Self::META, span, msg);
            return;
        }

        let replacement = if multiline {
            let base_indent = spaces(ctx.line_col(span.start).column);
            let body_indent = spaces(self.indentation_width);
            build_unless(ctx, node, &base_indent, &body_indent)
        } else {
            let predicate = node.predicate();
            let then_branch = node.statements().and_then(|s| s.body().first());
            let else_branch = else_clause
                .statements()
                .and_then(|s| s.body().first())
                .expect("guarded: exactly one statement");
            // Whitequark's uniform `:if` node type represents `unless` with
            // branches swapped relative to the real textual `then`/`else`
            // (`unless cond then a else b end` reads as `(if cond b a)`),
            // so the ternary's `?`/`:` slots are `else`/`then` here.
            Self::ternary_replacement(ctx, span, &predicate, Some(else_branch), then_branch)
        };

        ctx.report_with_fix(
            &Self::META,
            span,
            msg,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement)],
            },
        );
        self.ignored.push(span);
    }
}

impl Rule for OneLineConditional {
    const META: RuleMeta = RuleMeta {
        name: "Style/OneLineConditional",
        department: Department::Style,
        summary: "Favor the ternary operator (?:) or multi-line constructs over single-line if/then/else/end constructs.",
        explanation: "\
Checks for uses of `if/then/else/end` constructs on a single line.
A ternary operator (`?:`) or multi-line `if` is more readable.
`AlwaysCorrectToMultiline` config option can be set to `true` to autocorrect all offenses to
multi-line constructs. When `AlwaysCorrectToMultiline` is `false` (default case) the
autocorrect will first try converting them to ternary operators.

```ruby
# bad
if foo then bar else baz end

# bad
unless foo then baz else bar end

# good
foo ? bar : baz

# good
bar if foo

# good
if foo then bar end

# good
if foo
  bar
else
  baz
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[linter::ConfigOption {
            name: "AlwaysCorrectToMultiline",
            default: linter::ConfigDefault::Bool(false),
            allowed: &[],
            doc: "When `true`, autocorrects every offense to a multi-line construct instead of \
                  first trying a ternary operator.",
        }],
        blind_spots: "\
`node.parent.operator_keyword?`/`node.parent.send_type? && node.parent.operator_method?` (used \
to decide whether the ternary replacement needs an extra pair of wrapping parens) is \
approximated by reading the literal operator token back from source text between the relevant \
ancestor's span start and this node's own span start, since `Context::ancestors` carries only \
kind and span, not the full parent node. This matches every shape a real operator call can take \
here (including the `~`/`!` \"unary-looking infix\" reparse -- see the module doc) but would \
misidentify an explicitly parenthesized operator call (`a.+(if cond then x else y end)`) as not \
needing a wrap; no known spec exercises that spelling.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let always_multiline = options.bool("AlwaysCorrectToMultiline");
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .unwrap_or(2);
        Ok(Self {
            always_multiline,
            indentation_width: u32::try_from(indentation_width.max(0)).unwrap_or(2),
            ignored: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::IfNode => self.check_if(&node.as_if_node().expect("kind matched"), ctx),
            NodeKind::UnlessNode => {
                self.check_unless(&node.as_unless_node().expect("kind matched"), ctx);
            }
            _ => {}
        }
    }
}
