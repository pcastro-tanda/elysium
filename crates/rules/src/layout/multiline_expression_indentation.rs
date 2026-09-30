//! Shared port of RuboCop's `MultilineExpressionIndentation` mixin
//! (`lib/rubocop/cop/mixin/multiline_expression_indentation.rb`), used by
//! `Layout/MultilineMethodCallIndentation` and
//! `Layout/MultilineOperationIndentation`.
//!
//! Both cops navigate the tree in every direction: up to arbitrary
//! ancestors (and then *into* them), down a receiver chain and back up from
//! its base. Neither fits the engine's one-node-at-a-time `enter`, so both
//! build a [`Tree`] once per file in `file_start` and drive their own
//! `on_send` from it.
//!
//! [`Tree`] is also where the whitequark node model RuboCop is written
//! against is reconstructed from Prism's:
//!
//! * A call that carries a literal block is two whitequark nodes -- a
//!   `block` wrapping a `send` -- where Prism has one `CallNode` with a
//!   `BlockNode` in its `block` field. [`Tree`] stores that `CallNode`
//!   twice: once as the wrapper (spanning the block) and once, as its only
//!   "call" child, as the send (spanning [`call_span_excluding_block`]).
//! * Prism's `ArgumentsNode` and `StatementsNode` have no whitequark
//!   counterpart (a `begin` is only emitted for a *parenthesized* or
//!   multi-statement body, and the cops only ever ask about the
//!   parenthesized case), so their children are attached to the
//!   grandparent, which restores `hash.parent == send` and friends.
//! * A `LambdaNode` is whitequark's `(block (send nil :lambda) ...)`: a
//!   block whose `send_node` has no receiver and no dot.

use linter::{CommentKind, Context, Edit, OptionValue, RuleOptions};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Index of a node in a [`Tree`].
pub(crate) type Idx = usize;

#[derive(Clone, Copy)]
struct Entry<'pr> {
    node: Node<'pr>,
    /// True for the whitequark `send` view of a Prism `CallNode` that
    /// carries a literal block; the block wrapper is the same Prism node
    /// with this flag cleared.
    send_view: bool,
    parent: Option<Idx>,
    /// One past this node's last descendant, so a subtree is a range.
    end: Idx,
    span: Span,
}

/// The whitequark view of one file's Prism tree, in pre-order, with parent
/// links and subtree ranges.
pub(crate) struct Tree<'pr> {
    entries: Vec<Entry<'pr>>,
}

impl<'pr> Tree<'pr> {
    /// Builds the whole tree for `root`, once per file.
    pub(crate) fn build(root: &Node<'pr>) -> Self {
        let mut tree = Self { entries: Vec::new() };
        tree.add(root, None);
        tree
    }

    fn push(&mut self, node: Node<'pr>, send_view: bool, parent: Option<Idx>, span: Span) -> Idx {
        let i = self.entries.len();
        self.entries.push(Entry { node, send_view, parent, end: i + 1, span });
        i
    }

    fn close(&mut self, i: Idx) {
        self.entries[i].end = self.entries.len();
    }

    fn add(&mut self, node: &Node<'pr>, parent: Option<Idx>) {
        if matches!(node, Node::ArgumentsNode { .. } | Node::StatementsNode { .. }) {
            for_each_child(node, |child| self.add(child, parent));
            return;
        }
        if let Some(call) = node.as_call_node() {
            if let Some(block) = call.block().and_then(|b| b.as_block_node()) {
                let wrapper = self.push(*node, false, parent, node.span());
                let send = self.push(*node, true, Some(wrapper), call_span_excluding_block(&call));
                if let Some(receiver) = call.receiver() {
                    self.add(&receiver, Some(send));
                }
                if let Some(args) = call.arguments() {
                    for arg in &args.arguments() {
                        self.add(&arg, Some(send));
                    }
                }
                self.close(send);
                if let Some(params) = block.parameters() {
                    self.add(&params, Some(wrapper));
                }
                if let Some(body) = block.body() {
                    self.add(&body, Some(wrapper));
                }
                self.close(wrapper);
                return;
            }
        }
        let i = self.push(*node, false, parent, node.span());
        for_each_child(node, |child| self.add(child, Some(i)));
        self.close(i);
    }

    /// Every node, in pre-order.
    pub(crate) fn indices(&self) -> std::ops::Range<Idx> {
        0..self.entries.len()
    }

    /// The whitequark source range of `i`.
    pub(crate) fn span(&self, i: Idx) -> Span {
        self.entries[i].span
    }

    /// The underlying Prism node.
    pub(crate) fn node(&self, i: Idx) -> Node<'pr> {
        self.entries[i].node
    }

    /// The Prism node kind; the block wrapper and the send view of a call
    /// share it, so callers that care use [`Tree::any_block`].
    pub(crate) fn kind(&self, i: Idx) -> NodeKind {
        self.entries[i].node.kind()
    }

    pub(crate) fn parent(&self, i: Idx) -> Option<Idx> {
        self.entries[i].parent
    }

    /// `each_ancestor`: innermost first.
    pub(crate) fn ancestors(&self, i: Idx) -> Ancestors<'_, 'pr> {
        Ancestors { tree: self, next: self.entries[i].parent }
    }

    /// RuboCop-AST's `any_block_type?`: a `block`, `numblock`, `itblock` or
    /// lambda literal.
    pub(crate) fn any_block(&self, i: Idx) -> bool {
        let entry = &self.entries[i];
        if entry.send_view {
            return false;
        }
        match entry.node {
            Node::LambdaNode { .. } => true,
            Node::CallNode { .. } => self.literal_block(i).is_some(),
            _ => false,
        }
    }

    /// RuboCop-AST's `block_type?`: a literal block with ordinary (or no)
    /// parameters. A `_1`/`it` block is `numblock`/`itblock` instead, and a
    /// lambda literal is a plain `block`.
    pub(crate) fn block_type(&self, i: Idx) -> bool {
        if !self.any_block(i) {
            return false;
        }
        match self.literal_block(i).and_then(|b| b.parameters()) {
            Some(params) => !matches!(
                params,
                Node::NumberedParametersNode { .. } | Node::ItParametersNode { .. }
            ),
            None => true,
        }
    }

    /// The Prism `BlockNode` behind the block wrapper at `i`, if any.
    fn literal_block(&self, i: Idx) -> Option<ruby_ast::node::BlockNode<'pr>> {
        let entry = &self.entries[i];
        if entry.send_view {
            return None;
        }
        entry.node.as_call_node()?.block()?.as_block_node()
    }

    /// The delimiters of the block literal (or lambda) at `i`.
    fn block_delimiters(&self, i: Idx) -> Option<(Span, Span)> {
        let entry = &self.entries[i];
        if let Some(lambda) = entry.node.as_lambda_node() {
            return Some((lambda.opening_loc().span(), lambda.closing_loc().span()));
        }
        let block = self.literal_block(i)?;
        Some((block.opening_loc().span(), block.closing_loc().span()))
    }

    /// RuboCop-AST's `BlockNode#single_line?`: the *delimiters* are on one
    /// line, not the node as a whole.
    pub(crate) fn block_single_line(&self, ctx: &Context<'_>, i: Idx) -> bool {
        self.block_delimiters(i).is_some_and(|(open, close)| ctx.same_line(open, close))
    }

    /// The body of the block literal (or lambda) at `i`.
    pub(crate) fn block_body_span(&self, i: Idx) -> Option<Span> {
        let entry = &self.entries[i];
        if let Some(lambda) = entry.node.as_lambda_node() {
            return lambda.body().map(|b| b.span());
        }
        self.literal_block(i)?.body().map(|b| b.span())
    }

    /// The closing delimiter (`}` or `end`) of the block literal at `i`.
    pub(crate) fn block_end_span(&self, i: Idx) -> Option<Span> {
        self.block_delimiters(i).map(|(_, close)| close)
    }

    /// The `CallNode` at `i` when `i` is whitequark's `send`/`csend` (never
    /// the block wrapper).
    pub(crate) fn call(&self, i: Idx) -> Option<CallNode<'pr>> {
        if self.any_block(i) {
            return None;
        }
        self.entries[i].node.as_call_node()
    }

    /// RuboCop-AST's `call_type?`.
    pub(crate) fn is_call(&self, i: Idx) -> bool {
        self.call(i).is_some()
    }

    /// RuboCop-AST's `send_type?`: a call written without `&.`.
    pub(crate) fn is_send(&self, i: Idx) -> bool {
        self.call(i).is_some_and(|call| !call.is_safe_navigation())
    }

    /// `loc.dot`.
    pub(crate) fn dot(&self, i: Idx) -> Option<Span> {
        self.call(i)?.call_operator_loc().map(|loc| loc.span())
    }

    /// `loc.selector`.
    pub(crate) fn selector(&self, i: Idx) -> Option<Span> {
        self.call(i)?.message_loc().map(|loc| loc.span())
    }

    /// The whitequark `receiver`: a send's receiver, or -- for a block --
    /// its send's receiver.
    pub(crate) fn receiver(&self, i: Idx) -> Option<Idx> {
        let entry = &self.entries[i];
        let call = entry.node.as_call_node()?;
        let receiver = call.receiver()?;
        // A block wrapper's receiver belongs to the send it owns.
        let owner = if !entry.send_view && self.any_block(i) { i + 1 } else { i };
        self.child_with_span(owner, receiver.span())
    }

    /// The block wrapper a send belongs to (`MethodDispatchNode#block_node`).
    pub(crate) fn block_node(&self, i: Idx) -> Option<Idx> {
        if self.entries[i].send_view {
            self.entries[i].parent
        } else {
            None
        }
    }

    /// The `send` inside a block wrapper (`BlockNode#send_node`); `None`
    /// for a lambda literal, whose whitequark `(send nil :lambda)` has no
    /// Prism counterpart (and no receiver, dot or selector, which is all
    /// the callers ask about).
    pub(crate) fn send_node(&self, i: Idx) -> Option<Idx> {
        if self.any_block(i) && self.entries[i].node.as_call_node().is_some() {
            Some(i + 1)
        } else {
            None
        }
    }

    /// `node.each_descendant(:any_block).first`.
    pub(crate) fn first_descendant_block(&self, i: Idx) -> Option<Idx> {
        (i + 1..self.entries[i].end).find(|&j| self.any_block(j))
    }

    /// The direct children of `i`, in order.
    pub(crate) fn children(&self, i: Idx) -> Children<'_, 'pr> {
        Children { tree: self, next: i + 1, end: self.entries[i].end, owner: i }
    }

    /// The direct child of `owner` whose Prism node spans `span`.
    fn child_with_span(&self, owner: Idx, span: Span) -> Option<Idx> {
        let mut j = owner + 1;
        let end = self.entries[owner].end;
        while j < end {
            if self.entries[j].parent == Some(owner) {
                if self.entries[j].node.span() == span {
                    return Some(j);
                }
                j = self.entries[j].end;
            } else {
                j += 1;
            }
        }
        None
    }

    /// The argument spans of the call at `i`, in whitequark order (a
    /// block-pass `&blk` counts, as it does upstream).
    pub(crate) fn argument_spans(&self, i: Idx) -> Vec<Span> {
        let Some(call) = self.entries[i].node.as_call_node() else { return Vec::new() };
        let mut spans: Vec<Span> = call
            .arguments()
            .map(|args| args.arguments().iter().map(|arg| arg.span()).collect())
            .unwrap_or_default();
        if let Some(block) = call.block() {
            if block.as_block_argument_node().is_some() {
                spans.push(block.span());
            }
        }
        spans
    }

    /// RuboCop-AST's `hash_type?`: a braced literal or a braceless
    /// keyword-argument hash.
    pub(crate) fn is_hash(&self, i: Idx) -> bool {
        matches!(self.entries[i].node, Node::HashNode { .. } | Node::KeywordHashNode { .. })
    }

    /// RuboCop-AST's `begin_type?` as the cops use it: a parenthesized
    /// expression. Prism's `StatementsNode` never reaches the tree.
    pub(crate) fn is_grouped(&self, i: Idx) -> bool {
        matches!(self.entries[i].node, Node::ParenthesesNode { .. })
    }

    /// RuboCop-AST's `setter_method?` (`loc?(:operator)`): `a.b = c` or
    /// `a[b] = c`, not an `op_asgn`.
    pub(crate) fn is_setter(&self, i: Idx) -> bool {
        self.call(i).is_some_and(|call| call.equal_loc().is_some())
    }

    /// RuboCop's `Util#parentheses?` for a call: `loc.end` is a `)`, which
    /// rules out an index call's `]`.
    pub(crate) fn parenthesized(&self, ctx: &Context<'_>, i: Idx) -> Option<(Span, Span)> {
        let call = self.call(i)?;
        let (open, close) = (call.opening_loc()?.span(), call.closing_loc()?.span());
        (ctx.text(close) == b")").then_some((open, close))
    }
}

/// Iterator over a node's direct children, in source order.
pub(crate) struct Children<'tree, 'pr> {
    tree: &'tree Tree<'pr>,
    next: Idx,
    end: Idx,
    owner: Idx,
}

impl Iterator for Children<'_, '_> {
    type Item = Idx;

    fn next(&mut self) -> Option<Idx> {
        while self.next < self.end {
            let current = self.next;
            if self.tree.entries[current].parent == Some(self.owner) {
                self.next = self.tree.entries[current].end;
                return Some(current);
            }
            self.next += 1;
        }
        None
    }
}

/// Iterator over a node's ancestors, innermost first.
pub(crate) struct Ancestors<'tree, 'pr> {
    tree: &'tree Tree<'pr>,
    next: Option<Idx>,
}

impl Iterator for Ancestors<'_, '_> {
    type Item = Idx;

    fn next(&mut self) -> Option<Idx> {
        let current = self.next?;
        self.next = self.tree.entries[current].parent;
        Some(current)
    }
}

/// `IndentationWidth` resolution shared by both cops.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Widths {
    /// `Alignment#configured_indentation_width`.
    pub(crate) configured: i64,
    /// `Layout/IndentationWidth`'s own `Width`, added on top for a
    /// "special indentation" keyword continuation.
    pub(crate) layout: i64,
}

impl Widths {
    /// Reads `IndentationWidth` off the cop and its `Layout/IndentationWidth`
    /// peer.
    pub(crate) fn from_options(options: &RuleOptions) -> Self {
        let layout = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .unwrap_or(2);
        let configured =
            options.get("IndentationWidth").and_then(OptionValue::as_int).unwrap_or(layout);
        Self { configured, layout }
    }
}

/// `within_node?`.
pub(crate) fn within(inner: Span, outer: Span) -> bool {
    inner.start >= outer.start && inner.end <= outer.end
}

/// `Alignment#indentation`: the column of the first non-blank character on
/// the line `span` starts on.
pub(crate) fn indentation(ctx: &Context<'_>, span: Span) -> i64 {
    let line = ctx.line_text(ctx.line_col(span.start).line);
    let column = line.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(0);
    i64::try_from(column).unwrap_or(0)
}

/// A source range's `column`: RuboCop's character column, not a display
/// width.
pub(crate) fn column(ctx: &Context<'_>, offset: u32) -> i64 {
    i64::from(ctx.line_col(offset).column)
}

/// The 1-based line a range starts on.
pub(crate) fn line(ctx: &Context<'_>, span: Span) -> u32 {
    ctx.line_col(span.start).line
}

/// `dot.join(selector)`.
pub(crate) fn join(a: Span, b: Span) -> Span {
    Span::new(a.start.min(b.start), a.end.max(b.end))
}

/// `left_hand_side`: in a chain of calls, the top call node.
pub(crate) fn left_hand_side(tree: &Tree<'_>, mut lhs: Idx) -> Idx {
    while let Some(parent) = tree.parent(lhs) {
        if tree.is_call(parent) && tree.dot(parent).is_some() && !is_assignment_method(tree, parent)
        {
            lhs = parent;
        } else {
            break;
        }
    }
    lhs
}

/// RuboCop-AST's `assignment_method?`.
fn is_assignment_method(tree: &Tree<'_>, i: Idx) -> bool {
    let Some(call) = tree.call(i) else { return false };
    let name = call.name();
    let name = name.as_slice();
    name.ends_with(b"=") && !matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=" | b"<=>")
}

/// `correct_indentation`: one indentation width, doubled up for a
/// non-postfix prefix keyword.
pub(crate) fn correct_indentation(tree: &Tree<'_>, i: Idx, widths: Widths) -> i64 {
    match kw_node_with_special_indentation(tree, i) {
        Some(kw) if !postfix_conditional(tree, kw) => widths.configured + widths.layout,
        _ => widths.configured,
    }
}

/// `operation_description`.
pub(crate) fn operation_description(
    tree: &Tree<'_>,
    ctx: &Context<'_>,
    i: Idx,
    rhs: Span,
) -> String {
    if let Some(kw) = kw_node_with_special_indentation(tree, i) {
        return keyword_message_tail(tree, ctx, kw);
    }
    if part_of_assignment_rhs(tree, i, rhs).is_some() {
        return "an expression in an assignment".to_owned();
    }
    "an expression".to_owned()
}

/// `keyword_message_tail`.
fn keyword_message_tail(tree: &Tree<'_>, ctx: &Context<'_>, i: Idx) -> String {
    let keyword = keyword_span(tree, i)
        .map_or_else(String::new, |span| String::from_utf8_lossy(ctx.text(span)).into_owned());
    let kind = if keyword == "for" { "collection" } else { "condition" };
    let article = if keyword.starts_with('i') || keyword.starts_with('u') { "an" } else { "a" };
    format!("a {kind} in {article} `{keyword}` statement")
}

/// `loc.keyword` for the keyword ancestors this mixin looks at.
fn keyword_span(tree: &Tree<'_>, i: Idx) -> Option<Span> {
    let node = tree.node(i);
    match node {
        Node::IfNode { .. } => node.as_if_node()?.if_keyword_loc().map(|l| l.span()),
        Node::UnlessNode { .. } => Some(node.as_unless_node()?.keyword_loc().span()),
        Node::WhileNode { .. } => Some(node.as_while_node()?.keyword_loc().span()),
        Node::UntilNode { .. } => Some(node.as_until_node()?.keyword_loc().span()),
        Node::ForNode { .. } => Some(node.as_for_node()?.for_keyword_loc().span()),
        Node::ReturnNode { .. } => Some(node.as_return_node()?.keyword_loc().span()),
        _ => None,
    }
}

/// `KEYWORD_ANCESTOR_TYPES`: `for`, `if` (and `unless`), `while`, `until`,
/// `return`.
fn is_keyword_ancestor(tree: &Tree<'_>, i: Idx) -> bool {
    matches!(
        tree.kind(i),
        NodeKind::ForNode
            | NodeKind::IfNode
            | NodeKind::UnlessNode
            | NodeKind::WhileNode
            | NodeKind::UntilNode
            | NodeKind::ReturnNode
    )
}

/// `if_type? && ternary?`.
fn is_ternary(tree: &Tree<'_>, i: Idx) -> bool {
    tree.node(i).as_if_node().is_some_and(|node| node.if_keyword_loc().is_none())
}

/// `kw_node_with_special_indentation`.
pub(crate) fn kw_node_with_special_indentation(tree: &Tree<'_>, i: Idx) -> Option<Idx> {
    let span = tree.span(i);
    tree.ancestors(i).find(|&a| {
        is_keyword_ancestor(tree, a)
            && !is_ternary(tree, a)
            && indented_keyword_expression(tree, a).is_some_and(|expr| within(span, expr))
    })
}

/// `indented_keyword_expression`: a `for`'s collection, else the node's
/// first child (the condition, or a `return`'s first value).
pub(crate) fn indented_keyword_expression(tree: &Tree<'_>, i: Idx) -> Option<Span> {
    let node = tree.node(i);
    match node {
        Node::ForNode { .. } => Some(node.as_for_node()?.collection().span()),
        Node::IfNode { .. } => Some(node.as_if_node()?.predicate().span()),
        Node::UnlessNode { .. } => Some(node.as_unless_node()?.predicate().span()),
        Node::WhileNode { .. } => Some(node.as_while_node()?.predicate().span()),
        Node::UntilNode { .. } => Some(node.as_until_node()?.predicate().span()),
        Node::ReturnNode { .. } => node
            .as_return_node()?
            .arguments()
            .and_then(|a| a.arguments().iter().next())
            .map(|first| first.span()),
        _ => None,
    }
}

/// `postfix_conditional?`: an `if`/`unless` in modifier form.
fn postfix_conditional(tree: &Tree<'_>, i: Idx) -> bool {
    let node = tree.node(i);
    match node {
        Node::IfNode { .. } => node
            .as_if_node()
            .is_some_and(|n| n.if_keyword_loc().is_some() && n.end_keyword_loc().is_none()),
        Node::UnlessNode { .. } => {
            node.as_unless_node().is_some_and(|n| n.end_keyword_loc().is_none())
        }
        _ => false,
    }
}

/// Which calls `argument_in_method_call` accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArgKind {
    /// `:with_parentheses`.
    WithParentheses,
    /// `:with_or_without_parentheses`.
    WithOrWithoutParentheses,
}

/// `argument_in_method_call`.
pub(crate) fn argument_in_method_call(
    tree: &Tree<'_>,
    ctx: &Context<'_>,
    i: Idx,
    kind: ArgKind,
) -> Option<Idx> {
    let span = tree.span(i);
    for ancestor in tree.ancestors(i) {
        let is_block = tree.block_type(ancestor);
        if !is_block && !tree.is_send(ancestor) {
            continue;
        }
        if is_block {
            return None;
        }
        if tree.is_setter(ancestor) {
            continue;
        }
        if kind == ArgKind::WithParentheses && tree.parenthesized(ctx, ancestor).is_none() {
            continue;
        }
        if tree.argument_spans(ancestor).into_iter().any(|arg| within(span, arg)) {
            return Some(ancestor);
        }
    }
    None
}

/// `part_of_assignment_rhs`.
pub(crate) fn part_of_assignment_rhs(tree: &Tree<'_>, i: Idx, candidate: Span) -> Option<Idx> {
    for ancestor in tree.ancestors(i) {
        if disqualified_rhs(tree, ancestor, candidate) {
            return None;
        }
        if valid_rhs(tree, ancestor, candidate) {
            return Some(ancestor);
        }
    }
    None
}

/// `disqualified_rhs?`.
fn disqualified_rhs(tree: &Tree<'_>, ancestor: Idx, candidate: Span) -> bool {
    if matches!(
        tree.kind(ancestor),
        NodeKind::IfNode
            | NodeKind::UnlessNode
            | NodeKind::WhileNode
            | NodeKind::UntilNode
            | NodeKind::ForNode
            | NodeKind::ReturnNode
            | NodeKind::ArrayNode
    ) {
        return true;
    }
    if is_kwbegin(tree, ancestor) {
        return true;
    }
    tree.block_type(ancestor)
        && tree.block_body_span(ancestor).is_some_and(|body| within(candidate, body))
}

/// RuboCop-AST's `kwbegin_type?`: an explicit `begin ... end`, not the
/// implicit `BeginNode` Prism gives a `def` body with a `rescue`.
fn is_kwbegin(tree: &Tree<'_>, i: Idx) -> bool {
    tree.node(i).as_begin_node().is_some_and(|node| node.begin_keyword_loc().is_some())
}

/// `valid_rhs?`.
fn valid_rhs(tree: &Tree<'_>, ancestor: Idx, candidate: Span) -> bool {
    if tree.is_send(ancestor) {
        return tree.is_setter(ancestor)
            && last_argument_span(tree, ancestor).is_some_and(|rhs| within(candidate, rhs));
    }
    assignment_rhs(tree, ancestor).is_some_and(|rhs| within(candidate, rhs))
}

/// `assignment_rhs`, and RuboCop's `CheckAssignment.extract_rhs`: the value
/// of any assignment node, or a setter call's last argument. `None` when
/// `i` is not an assignment at all.
pub(crate) fn assignment_rhs(tree: &Tree<'_>, i: Idx) -> Option<Span> {
    let node = tree.node(i);
    if tree.call(i).is_some() {
        return tree.is_setter(i).then(|| last_argument_span(tree, i)).flatten();
    }
    let value = match node {
        Node::LocalVariableWriteNode { .. } => node.as_local_variable_write_node()?.value(),
        Node::InstanceVariableWriteNode { .. } => node.as_instance_variable_write_node()?.value(),
        Node::ClassVariableWriteNode { .. } => node.as_class_variable_write_node()?.value(),
        Node::GlobalVariableWriteNode { .. } => node.as_global_variable_write_node()?.value(),
        Node::ConstantWriteNode { .. } => node.as_constant_write_node()?.value(),
        Node::ConstantPathWriteNode { .. } => node.as_constant_path_write_node()?.value(),
        Node::MultiWriteNode { .. } => node.as_multi_write_node()?.value(),
        Node::LocalVariableOperatorWriteNode { .. } => {
            node.as_local_variable_operator_write_node()?.value()
        }
        Node::LocalVariableOrWriteNode { .. } => node.as_local_variable_or_write_node()?.value(),
        Node::LocalVariableAndWriteNode { .. } => node.as_local_variable_and_write_node()?.value(),
        Node::InstanceVariableOperatorWriteNode { .. } => {
            node.as_instance_variable_operator_write_node()?.value()
        }
        Node::InstanceVariableOrWriteNode { .. } => {
            node.as_instance_variable_or_write_node()?.value()
        }
        Node::InstanceVariableAndWriteNode { .. } => {
            node.as_instance_variable_and_write_node()?.value()
        }
        Node::ClassVariableOperatorWriteNode { .. } => {
            node.as_class_variable_operator_write_node()?.value()
        }
        Node::ClassVariableOrWriteNode { .. } => node.as_class_variable_or_write_node()?.value(),
        Node::ClassVariableAndWriteNode { .. } => node.as_class_variable_and_write_node()?.value(),
        Node::GlobalVariableOperatorWriteNode { .. } => {
            node.as_global_variable_operator_write_node()?.value()
        }
        Node::GlobalVariableOrWriteNode { .. } => node.as_global_variable_or_write_node()?.value(),
        Node::GlobalVariableAndWriteNode { .. } => {
            node.as_global_variable_and_write_node()?.value()
        }
        Node::ConstantOperatorWriteNode { .. } => node.as_constant_operator_write_node()?.value(),
        Node::ConstantOrWriteNode { .. } => node.as_constant_or_write_node()?.value(),
        Node::ConstantAndWriteNode { .. } => node.as_constant_and_write_node()?.value(),
        Node::ConstantPathOperatorWriteNode { .. } => {
            node.as_constant_path_operator_write_node()?.value()
        }
        Node::ConstantPathOrWriteNode { .. } => node.as_constant_path_or_write_node()?.value(),
        Node::ConstantPathAndWriteNode { .. } => node.as_constant_path_and_write_node()?.value(),
        Node::CallOperatorWriteNode { .. } => node.as_call_operator_write_node()?.value(),
        Node::CallOrWriteNode { .. } => node.as_call_or_write_node()?.value(),
        Node::CallAndWriteNode { .. } => node.as_call_and_write_node()?.value(),
        Node::IndexOperatorWriteNode { .. } => node.as_index_operator_write_node()?.value(),
        Node::IndexOrWriteNode { .. } => node.as_index_or_write_node()?.value(),
        Node::IndexAndWriteNode { .. } => node.as_index_and_write_node()?.value(),
        _ => return None,
    };
    Some(value.span())
}

/// A call's last whitequark argument.
fn last_argument_span(tree: &Tree<'_>, i: Idx) -> Option<Span> {
    tree.argument_spans(i).last().copied()
}

/// A call's first whitequark argument.
pub(crate) fn first_argument_span(tree: &Tree<'_>, i: Idx) -> Option<Span> {
    tree.argument_spans(i).first().copied()
}

/// `not_for_this_cop?`.
pub(crate) fn not_for_this_cop(tree: &Tree<'_>, ctx: &Context<'_>, i: Idx) -> bool {
    let span = tree.span(i);
    tree.ancestors(i).any(|a| tree.is_grouped(a) || inside_arg_list_parentheses(tree, ctx, span, a))
}

/// `inside_arg_list_parentheses?`.
pub(crate) fn inside_arg_list_parentheses(
    tree: &Tree<'_>,
    ctx: &Context<'_>,
    span: Span,
    ancestor: Idx,
) -> bool {
    if !tree.is_send(ancestor) {
        return false;
    }
    tree.parenthesized(ctx, ancestor)
        .is_some_and(|(open, close)| span.start > open.start && span.end < close.end)
}

/// RuboCop-AST's `operator_method?`.
pub(crate) fn is_operator_method(tree: &Tree<'_>, i: Idx) -> bool {
    let Some(call) = tree.call(i) else { return false };
    matches!(
        call.name().as_slice(),
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
            | b"[]"
            | b"[]="
            | b"!"
            | b"!="
            | b"!~"
            | b"`"
    )
}

/// RuboCop's `AlignmentCorrector.correct`, spelled out here rather than
/// reused from [`linter::shift_lines`] because these two cops hand it
/// ranges that *begin* with whitespace (a whole selector line, a block's
/// `end` line). Upstream's `calculate_range` picks the deletion side from
/// the character at the line's anchor alone -- deleting forward when it is
/// a space -- while `shift_lines` always deletes backwards on a range's
/// first line, which is only equivalent for ranges starting at a token.
pub(crate) fn align_correct(
    ctx: &Context<'_>,
    expr: Span,
    column_delta: i64,
    taboo: &[Span],
) -> Vec<Edit> {
    if column_delta == 0 || expr.start >= expr.end {
        return Vec::new();
    }
    // `block_comment_within?`
    if ctx
        .comments()
        .iter()
        .any(|comment| comment.kind == CommentKind::EmbDoc && expr.contains(comment.span))
    {
        return Vec::new();
    }
    let amount = u32::try_from(column_delta.unsigned_abs()).unwrap_or(0);
    let bytes = ctx.source().bytes();
    let start_line = ctx.line_col(expr.start).line;
    let end_line = ctx.line_col(expr.end - 1).line;
    let mut edits: Vec<Edit> = Vec::new();
    for physical in start_line..=end_line {
        let anchor =
            if physical == start_line { expr.start } else { ctx.line_span(physical).start };
        let at = bytes.get(anchor as usize).copied();
        let range = if column_delta > 0 {
            Span::empty(anchor)
        } else if at == Some(b' ') {
            Span::new(anchor, anchor + amount)
        } else {
            Span::new(anchor.saturating_sub(amount), anchor)
        };
        if taboo.iter().any(|t| t.contains(range)) {
            continue;
        }
        let edit = if column_delta > 0 {
            if at == Some(b'\n') {
                continue;
            }
            Edit::insert(anchor, vec![b' '; amount as usize])
        } else {
            let text = ctx.text(range);
            if text.is_empty() || !text.iter().all(|&b| b == b' ' || b == b'\t') {
                continue;
            }
            Edit::delete(range)
        };
        // A selector line and a single-line block's body/`end` line are
        // the same physical line; upstream's tree rewriter silently merges
        // the two identical actions.
        if !edits.contains(&edit) {
            edits.push(edit);
        }
    }
    edits
}
