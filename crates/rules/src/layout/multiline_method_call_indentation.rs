//! `Layout/MultilineMethodCallIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/multiline_method_call_indentation.rb` plus the
//! `MultilineExpressionIndentation` mixin (see
//! [`crate::layout::multiline_expression_indentation`]) and
//! `AlignmentCorrector`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

use super::multiline_expression_indentation::{
    align_correct, argument_in_method_call, assignment_rhs, column, correct_indentation,
    indentation, indented_keyword_expression, inside_arg_list_parentheses, is_operator_method,
    join, kw_node_with_special_indentation, left_hand_side, line, not_for_this_cop,
    operation_description, part_of_assignment_rhs, within, ArgKind, Idx, Tree, Widths,
};

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Align the continuation's dot with the first call's dot.
    Aligned,
    /// Indent the continuation one width past the expression's own line.
    Indented,
    /// Indent the continuation one width past the receiver.
    IndentedRelativeToReceiver,
}

/// Everything RuboCop keeps in instance variables while checking one call.
#[derive(Debug, Clone, Copy, Default)]
struct State {
    /// `@base`.
    base: Option<Span>,
    /// `@column_delta`.
    column_delta: i64,
}

/// Checks indentation of method calls with the dot operator that span more than one line.
#[derive(Debug, Clone)]
pub struct MultilineMethodCallIndentation {
    style: Style,
    widths: Widths,
    /// `@hash_pair_base_column`, which upstream never resets between
    /// nodes: once a `key:`-relative base has been computed, every later
    /// "no base" message in the same file measures against it.
    hash_pair_base_column: Option<i64>,
}

impl Rule for MultilineMethodCallIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/MultilineMethodCallIndentation",
        department: Department::Layout,
        summary:
            "Checks indentation of method calls with the dot operator that span more than one line.",
        explanation: "\
Checks the indentation of the method name part in method calls that span
more than one line.

```ruby
# EnforcedStyle: aligned (default)

# bad
while myvariable
.b
  # do something
end

# good
while myvariable
      .b
  # do something
end

# good
Thing.a
     .b
     .c
```

```ruby
# EnforcedStyle: indented

# good
while myvariable
  .b

  # do something
end
```

```ruby
# EnforcedStyle: indented_relative_to_receiver

# good
while myvariable
        .a
        .b

  # do something
end

# good
myvariable = Thing
               .a
               .b
               .c
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("aligned"),
                allowed: &["aligned", "indented", "indented_relative_to_receiver"],
                doc: "Aligns the continuation's dot with the chain's first dot \
                      (`aligned`), indents it one width past the expression's \
                      own line (`indented`), or one width past the receiver \
                      (`indented_relative_to_receiver`).",
            },
            ConfigOption {
                name: "IndentationWidth",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "Overrides `Layout/IndentationWidth`'s configured width. \
                      Only accepted when `EnforcedStyle` is not `aligned`.",
            },
        ],
        blind_spots: "\
Autocorrection's taboo-range protection (RuboCop's `AlignmentCorrector`
`inside_string_ranges`) only covers heredoc bodies, and only for the
block-body part of a correction; the offense range itself is shifted as a
raw range, exactly as upstream passes it.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "indented" => Style::Indented,
            "indented_relative_to_receiver" => Style::IndentedRelativeToReceiver,
            _ => Style::Aligned,
        };
        if style == Style::Aligned
            && options.get("IndentationWidth").and_then(OptionValue::as_int).is_some()
        {
            return Err(options.error(
                "IndentationWidth",
                "The `Layout/MultilineMethodCallIndentation` cop only accepts an \
                 `IndentationWidth` configuration parameter when `EnforcedStyle` is \
                 `indented`.",
            ));
        }
        Ok(Self { style, widths: Widths::from_options(options), hash_pair_base_column: None })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.hash_pair_base_column = None;
        let root = ctx.parsed().root();
        let tree = Tree::build(&root);
        for i in tree.indices() {
            if tree.is_call(i) {
                self.on_send(&tree, ctx, i);
            }
        }
    }
}

impl MultilineMethodCallIndentation {
    /// The mixin's `on_send`/`on_csend`.
    fn on_send(&mut self, tree: &Tree<'_>, ctx: &mut Context<'_>, node: Idx) {
        let Some(call) = tree.call(node) else { return };
        let Some(receiver) = tree.receiver(node) else { return };
        if call.name().as_slice() == b"[]" {
            return;
        }
        // `relevant_node?`: only method calls with a dot operator.
        if tree.dot(node).is_none() {
            return;
        }
        let lhs = left_hand_side(tree, receiver);
        let Some(rhs) = right_hand_side(tree, ctx, node) else { return };

        let mut state = State::default();
        let Some(range) = self.offending_range(tree, ctx, node, lhs, rhs, &mut state) else {
            return;
        };
        let message = self.message(tree, ctx, node, lhs, range, &state);
        match Self::build_fix(tree, ctx, node, range, state.column_delta) {
            Some(fix) => ctx.report_with_fix(&Self::META, range, message, fix),
            None => ctx.report(&Self::META, range, message),
        }
    }

    /// `offending_range`.
    fn offending_range(
        &mut self,
        tree: &Tree<'_>,
        ctx: &Context<'_>,
        node: Idx,
        lhs: Idx,
        rhs: Span,
        state: &mut State,
    ) -> Option<Span> {
        if !ctx.begins_its_line(rhs) {
            return None;
        }
        let pair_ancestor = find_pair_ancestor(tree, ctx, node);
        if let Some(pair) = pair_ancestor {
            if self.style == Style::Aligned {
                return Self::check_hash_pair_indentation(tree, ctx, node, lhs, rhs, state);
            }
            if self.style == Style::Indented && tree.is_hash(find_base_receiver(tree, node)) {
                return self.check_hash_pair_indented_style(tree, ctx, rhs, pair, state);
            }
        }
        // `skip_for_context?`
        let skip = if pair_ancestor.is_some() {
            inside_multiline_chain_arg(tree, ctx, node)
        } else {
            not_for_this_cop(tree, ctx, node)
        };
        if skip {
            return None;
        }
        self.check_regular_indentation(tree, ctx, node, lhs, rhs, state)
    }

    /// `check_hash_pair_indented_style`.
    fn check_hash_pair_indented_style(
        &mut self,
        tree: &Tree<'_>,
        ctx: &Context<'_>,
        rhs: Span,
        pair_ancestor: Idx,
        state: &mut State,
    ) -> Option<Span> {
        let key_column = tree
            .node(pair_ancestor)
            .as_assoc_node()
            .map_or(0, |pair| column(ctx, pair.key().location().span().start));
        let correct_column = key_column + self.widths.configured * 2;
        self.hash_pair_base_column = Some(key_column + self.widths.configured);
        calculate_column_delta_offense(ctx, rhs, correct_column, state)
    }

    /// `check_hash_pair_indentation`.
    fn check_hash_pair_indentation(
        tree: &Tree<'_>,
        ctx: &Context<'_>,
        node: Idx,
        lhs: Idx,
        rhs: Span,
        state: &mut State,
    ) -> Option<Span> {
        state.base = find_hash_pair_alignment_base(tree, node);
        if state.base.is_none() && inside_multiline_chain_arg(tree, ctx, node) {
            return None;
        }
        if state.base.is_none() {
            state.base = Some(
                first_dot_alignment_base(tree, ctx, node, rhs).unwrap_or_else(|| tree.span(lhs)),
            );
        }
        if aligned_with_first_line_dot(tree, ctx, node, rhs) {
            return None;
        }
        let base = state.base?;
        calculate_column_delta_offense(ctx, rhs, column(ctx, base.start), state)
    }

    /// `check_regular_indentation`.
    fn check_regular_indentation(
        &self,
        tree: &Tree<'_>,
        ctx: &Context<'_>,
        node: Idx,
        lhs: Idx,
        rhs: Span,
        state: &mut State,
    ) -> Option<Span> {
        state.base = self.alignment_base(tree, ctx, node, rhs);
        let correct_column = if let Some(base) = state.base {
            let mut parent = tree.parent(node);
            if parent.is_some_and(|p| tree.any_block(p)) {
                parent = parent.and_then(|p| tree.parent(p));
            }
            column(ctx, base.start) + self.extra_indentation(tree, parent)
        } else {
            indentation(ctx, tree.span(lhs)) + correct_indentation(tree, node, self.widths)
        };
        calculate_column_delta_offense(ctx, rhs, correct_column, state)
    }

    /// `extra_indentation`.
    fn extra_indentation(&self, tree: &Tree<'_>, parent: Option<Idx>) -> i64 {
        if self.style != Style::IndentedRelativeToReceiver {
            return 0;
        }
        let operator = parent.and_then(|p| match tree.node(p) {
            Node::SplatNode { .. } => tree.node(p).as_splat_node().map(|n| n.operator_loc().span()),
            Node::AssocSplatNode { .. } => {
                tree.node(p).as_assoc_splat_node().map(|n| n.operator_loc().span())
            }
            _ => None,
        });
        match operator {
            Some(operator) => self.widths.configured - i64::from(operator.len()),
            None => self.widths.configured,
        }
    }

    /// `alignment_base`.
    fn alignment_base(
        &self,
        tree: &Tree<'_>,
        ctx: &Context<'_>,
        node: Idx,
        rhs: Span,
    ) -> Option<Span> {
        match self.style {
            Style::Aligned => semantic_alignment_base(tree, ctx, node, rhs)
                .or_else(|| syntactic_alignment_base(tree, node, rhs)),
            Style::IndentedRelativeToReceiver => receiver_alignment_base(tree, ctx, node),
            Style::Indented => None,
        }
    }

    /// `message`.
    fn message(
        &self,
        tree: &Tree<'_>,
        ctx: &Context<'_>,
        node: Idx,
        lhs: Idx,
        rhs: Span,
        state: &State,
    ) -> String {
        if let Some(base) = state.base {
            let base_source = first_line_of(ctx, base);
            if self.style == Style::IndentedRelativeToReceiver {
                return format!(
                    "Indent `{}` {} spaces more than `{base_source}` on line {}.",
                    text(ctx, rhs),
                    self.widths.configured,
                    line(ctx, base)
                );
            }
            if self.style == Style::Aligned {
                return format!(
                    "Align `{}` with `{base_source}` on line {}.",
                    text(ctx, rhs),
                    line(ctx, base)
                );
            }
        }
        let (used, expected) = match self.hash_pair_base_column {
            Some(base_column) => (column(ctx, rhs.start) - base_column, self.widths.configured),
            None => (
                column(ctx, rhs.start) - indentation(ctx, tree.span(lhs)),
                correct_indentation(tree, node, self.widths),
            ),
        };
        let what = operation_description(tree, ctx, node, rhs);
        format!("Use {expected} (not {used}) spaces for indenting {what} spanning multiple lines.")
    }

    /// `autocorrect`: a call with a block moves its selector line, body and
    /// `end` line; anything else shifts the offending range itself.
    fn build_fix(
        tree: &Tree<'_>,
        ctx: &Context<'_>,
        node: Idx,
        range: Span,
        column_delta: i64,
    ) -> Option<Fix> {
        let mut edits = Vec::new();
        if let Some(block) = tree.block_node(node) {
            let selector_line = ctx.line_span(line(ctx, range));
            edits.extend(align_correct(ctx, selector_line, column_delta, &[]));
            if let Some(body) = tree.block_body_span(block) {
                let taboo = heredoc_taboo(ctx, tree, block);
                edits.extend(align_correct(ctx, body, column_delta, &taboo));
            }
            if let Some(end) = tree.block_end_span(block) {
                edits.extend(align_correct(ctx, ctx.whole_lines(end), column_delta, &[]));
            }
        } else {
            edits.extend(align_correct(ctx, range, column_delta, &[]));
        }
        if edits.is_empty() {
            None
        } else {
            Some(Fix { applicability: Applicability::Safe, edits })
        }
    }
}

/// `AlignmentCorrector`'s `inside_string_ranges` for a block body.
fn heredoc_taboo(ctx: &Context<'_>, tree: &Tree<'_>, block: Idx) -> Vec<Span> {
    let node = tree.node(block);
    node.as_call_node()
        .and_then(|call| call.block())
        .and_then(|b| b.as_block_node())
        .and_then(|b| b.body())
        .map_or_else(Vec::new, |body| linter::heredoc_bodies(ctx, &body))
}

/// `calculate_column_delta_offense`.
fn calculate_column_delta_offense(
    ctx: &Context<'_>,
    rhs: Span,
    correct_column: i64,
    state: &mut State,
) -> Option<Span> {
    state.column_delta = correct_column - column(ctx, rhs.start);
    (state.column_delta != 0).then_some(rhs)
}

/// `right_hand_side`.
fn right_hand_side(tree: &Tree<'_>, ctx: &Context<'_>, node: Idx) -> Option<Span> {
    let dot = tree.dot(node)?;
    let selector = tree.selector(node);
    let dot_text = ctx.text(dot);
    if matches!(dot_text, b"." | b"&.") {
        if let Some(selector) = selector {
            if ctx.same_line(dot, selector) {
                return Some(join(dot, selector));
            }
        }
    }
    if let Some(selector) = selector {
        return Some(selector);
    }
    // `implicit_call?`: `foo.()`.
    let call = tree.call(node)?;
    if call.name().as_slice() == b"call" {
        return call.opening_loc().map(|loc| join(dot, loc.span()));
    }
    None
}

/// `find_base_receiver`.
fn find_base_receiver(tree: &Tree<'_>, node: Idx) -> Idx {
    let mut base = node;
    while let Some(receiver) = tree.receiver(base) {
        base = receiver;
    }
    base
}

/// `find_pair_ancestor`.
fn find_pair_ancestor(tree: &Tree<'_>, ctx: &Context<'_>, node: Idx) -> Option<Idx> {
    let span = tree.span(node);
    for ancestor in tree.ancestors(node) {
        if tree.kind(ancestor) == NodeKind::AssocNode {
            return Some(ancestor);
        }
        if tree.is_grouped(ancestor) || inside_arg_list_parentheses(tree, ctx, span, ancestor) {
            return None;
        }
    }
    None
}

/// `unwrap_block_node`.
fn unwrap_block_node(tree: &Tree<'_>, node: Option<Idx>) -> Option<Idx> {
    let node = node?;
    if tree.any_block(node) {
        tree.send_node(node)
    } else {
        Some(node)
    }
}

/// `find_hash_pair_alignment_base`.
fn find_hash_pair_alignment_base(tree: &Tree<'_>, node: Idx) -> Option<Span> {
    let base_receiver = find_base_receiver(tree, tree.receiver(node)?);
    if !tree.is_hash(base_receiver) {
        return None;
    }
    let first_call = first_call_has_a_dot(tree, node)?;
    Some(join(tree.dot(first_call)?, tree.selector(first_call)?))
}

/// `first_dot_alignment_base`.
fn first_dot_alignment_base(
    tree: &Tree<'_>,
    ctx: &Context<'_>,
    node: Idx,
    rhs: Span,
) -> Option<Span> {
    if !starts_with_dot(ctx, rhs) {
        return None;
    }
    let first_call = first_call_has_a_dot(tree, node)?;
    let dot = tree.dot(first_call)?;
    if first_call == node {
        return None;
    }
    if let Some(after_block) = after_multiline_block_base(tree, ctx, first_call, node) {
        return Some(after_block);
    }
    let receiver = tree.receiver(first_call)?;
    if !ctx.same_line(dot, tree.span(receiver)) {
        return None;
    }
    Some(join(dot, tree.selector(first_call)?))
}

/// `after_multiline_block_base`.
fn after_multiline_block_base(
    tree: &Tree<'_>,
    ctx: &Context<'_>,
    first_call: Idx,
    node: Idx,
) -> Option<Span> {
    let block = tree.block_node(first_call)?;
    if tree.block_single_line(ctx, block) {
        return None;
    }
    let after_block = tree.parent(block)?;
    if !tree.is_call(after_block) || after_block == node {
        return None;
    }
    Some(join(tree.dot(after_block)?, tree.selector(after_block)?))
}

/// `inside_multiline_chain_arg?`.
fn inside_multiline_chain_arg(tree: &Tree<'_>, ctx: &Context<'_>, node: Idx) -> bool {
    let Some(enclosing) = find_enclosing_chain_call(tree, ctx, node) else { return false };
    let (Some(selector), Some(receiver)) = (tree.selector(enclosing), tree.receiver(enclosing))
    else {
        return false;
    };
    !ctx.same_line(selector, tree.span(receiver))
}

/// `find_enclosing_chain_call`.
fn find_enclosing_chain_call(tree: &Tree<'_>, ctx: &Context<'_>, node: Idx) -> Option<Idx> {
    let hash_ancestor = tree.parent(find_pair_ancestor(tree, ctx, node)?)?;
    let enclosing_call = tree.parent(hash_ancestor)?;
    // `hash_arg_in_chain?`
    if !tree.is_call(enclosing_call)
        || tree.receiver(enclosing_call) == Some(hash_ancestor)
        || tree.dot(enclosing_call).is_none()
    {
        return None;
    }
    Some(enclosing_call)
}

/// `aligned_with_first_line_dot?`.
fn aligned_with_first_line_dot(tree: &Tree<'_>, ctx: &Context<'_>, node: Idx, rhs: Span) -> bool {
    if !starts_with_dot(ctx, rhs) {
        return false;
    }
    let Some(first_call) = first_call_has_a_dot(tree, node) else { return false };
    if Some(first_call) == tree.receiver(node) {
        return false;
    }
    let Some(dot) = tree.dot(first_call) else { return false };
    line(ctx, dot) == line(ctx, tree.span(node)) && column(ctx, dot.start) == column(ctx, rhs.start)
}

/// `syntactic_alignment_base`.
fn syntactic_alignment_base(tree: &Tree<'_>, lhs: Idx, rhs: Span) -> Option<Span> {
    if let Some(base) = kw_node_with_special_indentation(tree, lhs) {
        return indented_keyword_expression(tree, base);
    }
    if let Some(base) = part_of_assignment_rhs(tree, lhs, rhs) {
        return assignment_rhs(tree, base);
    }
    operation_rhs(tree, lhs)
}

/// `operation_rhs`.
fn operation_rhs(tree: &Tree<'_>, node: Idx) -> Option<Span> {
    let receiver = tree.receiver(node)?;
    let receiver_span = tree.span(receiver);
    let operation = tree.ancestors(receiver).find(|&a| {
        tree.is_send(a)
            && is_operator_method(tree, a)
            && super::multiline_expression_indentation::first_argument_span(tree, a)
                .is_some_and(|first| within(receiver_span, first))
    })?;
    super::multiline_expression_indentation::first_argument_span(tree, operation)
}

/// `semantic_alignment_base`.
fn semantic_alignment_base(
    tree: &Tree<'_>,
    ctx: &Context<'_>,
    node: Idx,
    rhs: Span,
) -> Option<Span> {
    if !starts_with_dot(ctx, rhs) {
        return None;
    }
    let node = semantic_alignment_node(tree, ctx, node)?;
    Some(join(tree.dot(node)?, tree.selector(node)?))
}

/// `semantic_alignment_node`.
fn semantic_alignment_node(tree: &Tree<'_>, ctx: &Context<'_>, node: Idx) -> Option<Idx> {
    if argument_in_method_call(tree, ctx, node, ArgKind::WithParentheses).is_some() {
        return None;
    }
    get_dot_right_above(tree, ctx, node)
        .or_else(|| find_multiline_block_chain_node(tree, ctx, node))
        .or_else(|| first_call_alignment_node(tree, ctx, node))
}

/// `first_call_alignment_node`.
fn first_call_alignment_node(tree: &Tree<'_>, ctx: &Context<'_>, node: Idx) -> Option<Idx> {
    let node = first_call_has_a_dot(tree, node)?;
    let base_receiver = Some(find_base_receiver(tree, node));
    if method_on_receiver_last_line(tree, ctx, node, base_receiver, NodeKind::ArrayNode) {
        return Some(node);
    }
    let dot = tree.dot(node)?;
    if line(ctx, dot) != line(ctx, tree.span(node)) {
        return None;
    }
    if method_on_receiver_last_line(tree, ctx, node, base_receiver, NodeKind::ParenthesesNode) {
        return None;
    }
    Some(node)
}

/// `method_on_receiver_last_line?`.
fn method_on_receiver_last_line(
    tree: &Tree<'_>,
    ctx: &Context<'_>,
    node: Idx,
    base_receiver: Option<Idx>,
    kind: NodeKind,
) -> bool {
    let Some(base_receiver) = base_receiver else { return false };
    let Some(dot) = tree.dot(node) else { return false };
    let end = Span::empty(tree.span(base_receiver).end);
    ctx.same_line(dot, end) && tree.kind(base_receiver) == kind
}

/// `get_dot_right_above`.
fn get_dot_right_above(tree: &Tree<'_>, ctx: &Context<'_>, node: Idx) -> Option<Idx> {
    let own_dot = tree.dot(node)?;
    let own_line = line(ctx, own_dot);
    let own_column = column(ctx, own_dot.start);
    tree.ancestors(node).find(|&a| {
        tree.dot(a).is_some_and(|dot| {
            line(ctx, dot) + 1 == own_line && column(ctx, dot.start) == own_column
        })
    })
}

/// `find_multiline_block_chain_node`.
fn find_multiline_block_chain_node(tree: &Tree<'_>, ctx: &Context<'_>, node: Idx) -> Option<Idx> {
    if tree.block_node(node).is_some() {
        return find_continuation_node(tree, ctx, node);
    }
    handle_descendant_block(tree, ctx, node)
}

/// `find_continuation_node`.
fn find_continuation_node(tree: &Tree<'_>, ctx: &Context<'_>, node: Idx) -> Option<Idx> {
    let receiver = tree.receiver(node)?;
    if single_line_block_receiver(tree, ctx, receiver) {
        return leftmost_call_on_same_line(tree, ctx, receiver);
    }
    if !tree.is_call(receiver) {
        return None;
    }
    let dot = tree.dot(receiver)?;
    let inner = tree.receiver(receiver)?;
    let block = tree.block_node(node)?;
    if tree.is_grouped(inner) && tree.block_single_line(ctx, block) {
        return Some(receiver);
    }
    if line(ctx, dot) <= ctx.last_line(tree.span(inner)) {
        return None;
    }
    Some(receiver)
}

/// `single_line_block_receiver?`.
fn single_line_block_receiver(tree: &Tree<'_>, ctx: &Context<'_>, receiver: Idx) -> bool {
    // `BlockNode#single_line?` compares the block's delimiters, not the
    // whole node, which may start on the receiver's line.
    tree.any_block(receiver) && tree.block_single_line(ctx, receiver)
}

/// `leftmost_call_on_same_line`.
fn leftmost_call_on_same_line(tree: &Tree<'_>, ctx: &Context<'_>, node: Idx) -> Option<Idx> {
    let mut current = unwrap_block_node(tree, Some(node))?;
    loop {
        let Some(current_dot) = tree.dot(current) else { return Some(current) };
        let Some(receiver) = unwrap_block_node(tree, tree.receiver(current)) else {
            return Some(current);
        };
        let same_line = tree.is_call(receiver)
            && tree.dot(receiver).is_some_and(|dot| line(ctx, dot) == line(ctx, current_dot));
        if !same_line {
            return Some(current);
        }
        current = receiver;
    }
}

/// `handle_descendant_block`.
fn handle_descendant_block(tree: &Tree<'_>, ctx: &Context<'_>, node: Idx) -> Option<Idx> {
    let receiver = tree.receiver(node)?;
    if single_line_block_receiver(tree, ctx, receiver) {
        return leftmost_call_on_same_line(tree, ctx, receiver);
    }
    let block = tree.first_descendant_block(node)?;
    if tree.block_single_line(ctx, block) {
        return None;
    }
    if tree.is_call(receiver) {
        Some(receiver)
    } else {
        tree.parent(block)
    }
}

/// `receiver_alignment_base`.
fn receiver_alignment_base(tree: &Tree<'_>, ctx: &Context<'_>, node: Idx) -> Option<Span> {
    if let Some(base) = find_hash_method_base_in_receiver_chain(tree, ctx, node) {
        return Some(base);
    }
    let first_call = first_call_has_a_dot(tree, node)?;
    Some(tree.span(tree.receiver(first_call)?))
}

/// `find_hash_method_base_in_receiver_chain`.
fn find_hash_method_base_in_receiver_chain(
    tree: &Tree<'_>,
    ctx: &Context<'_>,
    node: Idx,
) -> Option<Span> {
    let mut receiver_chain = unwrap_block_node(tree, tree.receiver(node));
    while let Some(current) = receiver_chain {
        if !tree.is_call(current) {
            return None;
        }
        let base_receiver = unwrap_block_node(tree, tree.receiver(current));
        let is_base = base_receiver.is_some_and(|base| tree.is_hash(base))
            || method_on_receiver_last_line(
                tree,
                ctx,
                current,
                base_receiver,
                NodeKind::ParenthesesNode,
            );
        if is_base {
            return Some(join(tree.dot(current)?, tree.selector(current)?));
        }
        receiver_chain = base_receiver;
    }
    None
}

/// `first_call_has_a_dot`.
fn first_call_has_a_dot(tree: &Tree<'_>, node: Idx) -> Option<Idx> {
    let base = find_base_receiver(tree, node);
    let mut current = tree.parent(base)?;
    while tree.dot(current).is_none() {
        current = tree.parent(current)?;
    }
    Some(current)
}

/// Whether the offending range begins with a `.` or `&.`.
fn starts_with_dot(ctx: &Context<'_>, span: Span) -> bool {
    matches!(ctx.text(span).first(), Some(b'.' | b'&'))
}

/// The range's source text.
fn text(ctx: &Context<'_>, span: Span) -> String {
    String::from_utf8_lossy(ctx.text(span)).into_owned()
}

/// `base_source`: the base range's first physical line.
fn first_line_of(ctx: &Context<'_>, span: Span) -> String {
    let bytes = ctx.text(span);
    let end = bytes.iter().position(|&b| b == b'\n').unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
