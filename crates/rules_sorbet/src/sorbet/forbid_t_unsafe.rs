//! `Sorbet/ForbidTUnsafe`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_t_unsafe.rb` (with its
//! `RBSAssertionCorrection` mixin and `RBSParser.rbs_annotation_after`).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, const_name};
use ruby_ast::node::CallNode;
use ruby_ast::{each_descendant, for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Do not use `T.unsafe`.";

/// Disallows using `T.unsafe` anywhere. Set `AutocorrectToRBS: true` to
/// replace supported calls with RBS inline comments.
#[derive(Debug, Clone)]
pub struct ForbidTUnsafe {
    autocorrect_to_rbs: bool,
    indentation_width: usize,
}

impl Rule for ForbidTUnsafe {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidTUnsafe",
        department: Department::Sorbet,
        summary: "Forbid usage of T.unsafe.",
        explanation: "Disallows using `T.unsafe` anywhere.\nSet `AutocorrectToRBS: true` to \
                      replace supported calls with RBS inline comments.\n\n```ruby\n# bad\n\
                      T.unsafe(foo)\n\n# good\nfoo #: as untyped\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "AutocorrectToRBS",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Replace supported `T.unsafe` calls with RBS inline comments.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        // `Alignment#configured_indentation_width`: the cop's own
        // `IndentationWidth` (absent), else `Layout/IndentationWidth: Width`, else 2.
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .and_then(|width| usize::try_from(width).ok())
            .unwrap_or(2);
        Ok(Self { autocorrect_to_rbs: options.bool("AutocorrectToRBS"), indentation_width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // `(send (const nil? :T) :unsafe _)`
        if call.name().as_slice() != b"unsafe"
            || call.is_safe_navigation()
            || !call.receiver().is_some_and(|receiver| is_const_t(&receiver))
        {
            return;
        }
        let arguments = argument_list(&call);
        if arguments.len() != 1 {
            return;
        }

        let span = send_span(&call);
        let edits = self.autocorrect_t_unsafe_to_rbs(ctx, node, &call, span, &arguments[0]);
        match edits {
            Some(edits) if !edits.is_empty() => ctx.report_with_fix(
                &Self::META,
                span,
                MSG,
                Fix { applicability: Applicability::Unsafe, edits },
            ),
            _ => ctx.report(&Self::META, span, MSG),
        }
    }
}

/// The call's `source_range` in whitequark: a block's attached call stops
/// before the block.
fn send_span(call: &CallNode<'_>) -> Span {
    if call.block().is_some_and(|block| block.as_block_node().is_some()) {
        call_span_excluding_block(call)
    } else {
        call.as_node().span()
    }
}

fn is_const_t(node: &Node<'_>) -> bool {
    node.as_constant_read_node().is_some_and(|constant| constant.name().as_slice() == b"T")
}

/// `send` arguments as whitequark sees them: a `&blk` is a `block_pass` child.
fn argument_list<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut args: Vec<Node<'pr>> =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block().filter(|block| block.as_block_argument_node().is_some()) {
        args.push(block);
    }
    args
}

fn same(a: &Node<'_>, b: &Node<'_>) -> bool {
    a.span() == b.span() && a.kind() == b.kind()
}

/// The value child of an assignment node (`parent.assignment? &&
/// parent.children.last`), including setter calls (`self.foo = x`, `a[0] = x`),
/// which are `assignment?` through `MethodDispatchNode#setter_method?`.
fn assigned_value<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => Some(node.as_local_variable_write_node()?.value()),
        NodeKind::InstanceVariableWriteNode => {
            Some(node.as_instance_variable_write_node()?.value())
        }
        NodeKind::ClassVariableWriteNode => Some(node.as_class_variable_write_node()?.value()),
        NodeKind::GlobalVariableWriteNode => Some(node.as_global_variable_write_node()?.value()),
        NodeKind::ConstantWriteNode => Some(node.as_constant_write_node()?.value()),
        NodeKind::ConstantPathWriteNode => Some(node.as_constant_path_write_node()?.value()),
        NodeKind::LocalVariableOrWriteNode => Some(node.as_local_variable_or_write_node()?.value()),
        NodeKind::LocalVariableAndWriteNode => {
            Some(node.as_local_variable_and_write_node()?.value())
        }
        NodeKind::LocalVariableOperatorWriteNode => {
            Some(node.as_local_variable_operator_write_node()?.value())
        }
        NodeKind::InstanceVariableOrWriteNode => {
            Some(node.as_instance_variable_or_write_node()?.value())
        }
        NodeKind::InstanceVariableAndWriteNode => {
            Some(node.as_instance_variable_and_write_node()?.value())
        }
        NodeKind::InstanceVariableOperatorWriteNode => {
            Some(node.as_instance_variable_operator_write_node()?.value())
        }
        NodeKind::ClassVariableOrWriteNode => Some(node.as_class_variable_or_write_node()?.value()),
        NodeKind::ClassVariableAndWriteNode => {
            Some(node.as_class_variable_and_write_node()?.value())
        }
        NodeKind::ClassVariableOperatorWriteNode => {
            Some(node.as_class_variable_operator_write_node()?.value())
        }
        NodeKind::GlobalVariableOrWriteNode => {
            Some(node.as_global_variable_or_write_node()?.value())
        }
        NodeKind::GlobalVariableAndWriteNode => {
            Some(node.as_global_variable_and_write_node()?.value())
        }
        NodeKind::GlobalVariableOperatorWriteNode => {
            Some(node.as_global_variable_operator_write_node()?.value())
        }
        NodeKind::ConstantOrWriteNode => Some(node.as_constant_or_write_node()?.value()),
        NodeKind::ConstantAndWriteNode => Some(node.as_constant_and_write_node()?.value()),
        NodeKind::ConstantOperatorWriteNode => {
            Some(node.as_constant_operator_write_node()?.value())
        }
        NodeKind::ConstantPathOrWriteNode => Some(node.as_constant_path_or_write_node()?.value()),
        NodeKind::ConstantPathAndWriteNode => Some(node.as_constant_path_and_write_node()?.value()),
        NodeKind::ConstantPathOperatorWriteNode => {
            Some(node.as_constant_path_operator_write_node()?.value())
        }
        NodeKind::CallOrWriteNode => Some(node.as_call_or_write_node()?.value()),
        NodeKind::CallAndWriteNode => Some(node.as_call_and_write_node()?.value()),
        NodeKind::CallOperatorWriteNode => Some(node.as_call_operator_write_node()?.value()),
        NodeKind::IndexOrWriteNode => Some(node.as_index_or_write_node()?.value()),
        NodeKind::IndexAndWriteNode => Some(node.as_index_and_write_node()?.value()),
        NodeKind::IndexOperatorWriteNode => Some(node.as_index_operator_write_node()?.value()),
        NodeKind::MultiWriteNode => Some(node.as_multi_write_node()?.value()),
        NodeKind::CallNode => {
            let call = node.as_call_node()?;
            if !call.is_attribute_write() {
                return None;
            }
            call.arguments()?.arguments().iter().last()
        }
        _ => None,
    }
}

/// The path of Prism ancestors (outermost first) leading to `target`.
fn find_path<'pr>(node: &Node<'pr>, target: &Node<'pr>, path: &mut Vec<Node<'pr>>) -> bool {
    if same(node, target) {
        return true;
    }
    let (span, wanted) = (node.span(), target.span());
    if span.start > wanted.start || span.end < wanted.end {
        return false;
    }
    path.push(*node);
    let mut found = false;
    for_each_child(node, |child| {
        if !found && find_path(child, target, path) {
            found = true;
        }
    });
    if !found {
        path.pop();
    }
    found
}

/// The trailing RBS annotation (`#: ...`) attached to the expression ending
/// at `end`, as `RBSParser.rbs_annotation_after`.
fn has_rbs_annotation_after(ctx: &Context<'_>, span: Span) -> bool {
    let last_line = ctx.last_line(span);
    let source = ctx.source().bytes();
    ctx.comments().iter().any(|comment| {
        if comment.line != last_line || comment.span.start < span.end {
            return false;
        }
        let gap = &source[span.end as usize..comment.span.start as usize];
        if !gap.iter().all(|byte| matches!(byte, b' ' | b'\t')) {
            return false;
        }
        let text = ctx.text(comment.span);
        text.starts_with(b"#:") && !text[2..].trim_ascii().is_empty()
    })
}

impl ForbidTUnsafe {
    /// `autocorrect_t_unsafe_to_rbs`: `None` when no correction applies.
    fn autocorrect_t_unsafe_to_rbs(
        &self,
        ctx: &Context<'_>,
        node: &Node<'_>,
        call: &CallNode<'_>,
        span: Span,
        first_argument: &Node<'_>,
    ) -> Option<Vec<Edit>> {
        let expression = String::from_utf8_lossy(ctx.text(first_argument.span())).into_owned();
        let annotation = "as untyped";

        let mut path = Vec::new();
        let root = ctx.parsed().root();
        if !find_path(&root, node, &mut path) {
            return None;
        }
        // Whitequark has no `args` node.
        path.retain(|ancestor| ancestor.as_arguments_node().is_none());

        if self.rbs_assertion_autocorrectable(ctx, span, &path, node) {
            return Some(vec![Edit::replace(
                span,
                format!("{expression} #: {annotation}").into_bytes(),
            )]);
        }
        if !self.nested_rbs_assertion_autocorrectable(ctx, span, &path) {
            return None;
        }
        let context = call_argument_context(ctx, call, span, &path, node)?;
        Some(self.autocorrect_argument_list(ctx, span, &expression, annotation, &context))
    }

    fn rbs_assertion_autocorrectable(
        &self,
        ctx: &Context<'_>,
        span: Span,
        path: &[Node<'_>],
        node: &Node<'_>,
    ) -> bool {
        if !self.autocorrect_to_rbs || !assertion_ends_line(ctx, span) {
            return false;
        }
        if has_rbs_annotation_after(ctx, span) {
            return false;
        }
        // `assertion_statement(node, allow_assignment: true)`
        let statement = match path.last() {
            Some(parent) if assigned_value(parent).is_some_and(|value| same(&value, node)) => {
                parent.span()
            }
            _ => span,
        };
        let line = ctx.line_col(statement.start).line;
        let text = ctx.line_text(line);
        text.iter().position(|byte| !byte.is_ascii_whitespace()).is_some_and(|index| {
            ctx.line_span(line).start as usize + index == statement.start as usize
        })
    }

    fn nested_rbs_assertion_autocorrectable(
        &self,
        ctx: &Context<'_>,
        span: Span,
        path: &[Node<'_>],
    ) -> bool {
        if !self.autocorrect_to_rbs || !ctx.is_single_line(span) {
            return false;
        }
        if inside_single_line_block(ctx, path) {
            return false;
        }
        !has_rbs_annotation_after(ctx, span)
    }

    fn autocorrect_argument_list(
        &self,
        ctx: &Context<'_>,
        node_span: Span,
        expression: &str,
        annotation: &str,
        context: &ArgumentContext<'_>,
    ) -> Vec<Edit> {
        let container = context.call_span;
        let line = ctx.line_col(container.start).line;
        let leading: Vec<u8> = ctx
            .line_text(line)
            .iter()
            .copied()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .collect();
        let indentation = String::from_utf8_lossy(&leading).into_owned();
        let width = " ".repeat(self.indentation_width);
        let item_indentation = format!("{indentation}{width}");
        let splat = matches!(context.target.kind(), NodeKind::SplatNode | NodeKind::AssocSplatNode);
        let target_index =
            context.items.iter().position(|item| same(item, &context.target)).unwrap_or(0);
        let mut replacement = format!("{expression} #: {annotation}");
        if splat {
            let nested_indentation = format!("{item_indentation}{width}");
            replacement = format!("(\n{nested_indentation}{replacement}\n{item_indentation})");
        } else if target_index + 1 < context.items.len() {
            replacement = format!("{expression}, #: {annotation}");
        }

        let mut edits = vec![Edit::replace(node_span, replacement.into_bytes())];
        let (first, last) = (context.items[0], context.items[context.items.len() - 1]);
        edits.push(Edit::replace(
            Span::new(context.open_end, first.span().start),
            format!("\n{item_indentation}").into_bytes(),
        ));
        for pair in context.items.windows(2) {
            let (item, next_item) = (&pair[0], &pair[1]);
            let separator = if same(item, &context.target) && !splat { "" } else { "," };
            edits.push(Edit::replace(
                Span::new(item.span().end, next_item.span().start),
                format!("{separator}\n{item_indentation}").into_bytes(),
            ));
        }
        edits.push(Edit::replace(
            Span::new(last.span().end, context.close_start),
            format!("\n{indentation}").into_bytes(),
        ));
        edits
    }
}

/// `[:arguments, call, items, target]` of `call_argument_context`.
struct ArgumentContext<'pr> {
    call_span: Span,
    /// `call.loc.begin.end`
    open_end: u32,
    /// `call.loc.end.begin`
    close_start: u32,
    items: Vec<Node<'pr>>,
    target: Node<'pr>,
}

fn is_braceless_hash(node: &Node<'_>) -> bool {
    node.as_keyword_hash_node().is_some()
}

fn call_argument_context<'pr>(
    ctx: &Context<'_>,
    node_call: &CallNode<'_>,
    node_span: Span,
    path: &[Node<'pr>],
    node: &Node<'_>,
) -> Option<ArgumentContext<'pr>> {
    let _ = node_span;
    let parent = *path.last()?;
    let in_arguments = |call: &CallNode<'pr>, candidate: &Node<'_>| {
        argument_list(call).iter().any(|argument| same(argument, candidate))
    };
    let (call, target): (CallNode<'pr>, Node<'pr>) = if let Some(call) = parent.as_call_node() {
        let target = argument_list(&call).into_iter().find(|argument| same(argument, node))?;
        (call, target)
    } else if parent.as_splat_node().is_some() {
        let call = path.get(path.len().checked_sub(2)?)?.as_call_node()?;
        if !in_arguments(&call, &parent) {
            return None;
        }
        (call, parent)
    } else if parent.as_assoc_splat_node().is_some() {
        if argument_list(node_call).first().is_some_and(is_braceless_hash) {
            return None;
        }
        let hash = *path.get(path.len().checked_sub(2)?)?;
        if !is_braceless_hash(&hash) {
            return None;
        }
        let call = path.get(path.len().checked_sub(3)?)?.as_call_node()?;
        if !in_arguments(&call, &hash) {
            return None;
        }
        (call, parent)
    } else if parent.as_assoc_node().is_some_and(|pair| same(&pair.value(), node)) {
        let hash = *path.get(path.len().checked_sub(2)?)?;
        if !is_braceless_hash(&hash) {
            return None;
        }
        let call = path.get(path.len().checked_sub(3)?)?.as_call_node()?;
        if !in_arguments(&call, &hash) {
            return None;
        }
        (call, parent)
    } else {
        return None;
    };

    let call_span = send_span(&call);
    let (open, close) = (call.opening_loc()?, call.closing_loc()?);
    if !ctx.is_single_line(call_span) {
        return None;
    }
    if ctx
        .comments()
        .iter()
        .any(|comment| comment.span.start >= call_span.start && comment.span.end <= call_span.end)
    {
        return None;
    }
    if !one_matching_assertion(&call, node_call) {
        return None;
    }

    let mut items: Vec<Node<'pr>> = Vec::new();
    for argument in argument_list(&call) {
        if is_braceless_hash(&argument) {
            for_each_child(&argument, |child| items.push(*child));
        } else {
            items.push(argument);
        }
    }
    if !items.iter().any(|item| same(item, &target)) {
        return None;
    }
    Some(ArgumentContext {
        call_span,
        open_end: open.span().end,
        close_start: close.span().start,
        items,
        target,
    })
}

/// `container.each_node(:send).count { T.<method> } == 1`; the container's
/// block (if any) is a parent in whitequark, so it is not walked.
fn one_matching_assertion(container: &CallNode<'_>, node: &CallNode<'_>) -> bool {
    let mut count = 0;
    let mut visit = |candidate: &Node<'_>| {
        let Some(call) = candidate.as_call_node() else { return };
        if !call.is_safe_navigation()
            && call.name().as_slice() == node.name().as_slice()
            && call
                .receiver()
                .is_some_and(|receiver| const_name(&receiver).is_some_and(|name| name == "T"))
        {
            count += 1;
        }
    };
    // The container itself, then its receiver, arguments and `&blk`.
    visit(&container.as_node());
    let mut pieces: Vec<Node<'_>> = container.receiver().into_iter().collect();
    pieces.extend(argument_list(container));
    for piece in &pieces {
        visit(piece);
        each_descendant(piece, &mut visit);
    }
    count == 1
}

/// `assertion_ends_line?`: only horizontal whitespace, then end of input, a
/// newline or a comment.
fn assertion_ends_line(ctx: &Context<'_>, span: Span) -> bool {
    let source = ctx.source().bytes();
    let mut end = span.end as usize;
    while matches!(source.get(end), Some(b' ' | b'\t')) {
        end += 1;
    }
    matches!(source.get(end), None | Some(b'\n' | b'\r' | b'#'))
}

/// `node.each_ancestor.any? { |a| a.type?(:block, :numblock) && a.single_line? }`.
/// Whitequark's `block` node spans its call; `itblock` is not included.
fn inside_single_line_block(ctx: &Context<'_>, path: &[Node<'_>]) -> bool {
    path.iter().any(|ancestor| {
        let (block_span, is_it) = match ancestor.kind() {
            NodeKind::LambdaNode => {
                let Some(lambda) = ancestor.as_lambda_node() else { return false };
                let is_it =
                    lambda.parameters().is_some_and(|p| p.as_it_parameters_node().is_some());
                (ancestor.span(), is_it)
            }
            NodeKind::CallNode | NodeKind::SuperNode | NodeKind::ForwardingSuperNode => {
                let block = match ancestor.kind() {
                    NodeKind::CallNode => ancestor.as_call_node().and_then(|c| c.block()),
                    NodeKind::SuperNode => ancestor.as_super_node().and_then(|c| c.block()),
                    _ => None,
                };
                let is_it = match (block, ancestor.as_forwarding_super_node()) {
                    (Some(block), _) => {
                        let Some(block) = block.as_block_node() else { return false };
                        block.parameters().is_some_and(|p| p.as_it_parameters_node().is_some())
                    }
                    (None, Some(forwarding)) => {
                        let Some(block) = forwarding.block() else { return false };
                        block.parameters().is_some_and(|p| p.as_it_parameters_node().is_some())
                    }
                    (None, None) => return false,
                };
                (ancestor.span(), is_it)
            }
            _ => return false,
        };
        !is_it && ctx.is_single_line(block_span)
    })
}
