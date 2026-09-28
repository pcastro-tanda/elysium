//! `Lint/UselessTimes`, ported from RuboCop's
//! `lib/rubocop/cop/lint/useless_times.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::BlockNode;
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks for useless `Integer#times` calls.
#[derive(Debug, Clone)]
pub struct UselessTimes;

impl Rule for UselessTimes {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UselessTimes",
        department: Department::Lint,
        summary: "Checks for useless `Integer#times` calls.",
        explanation: "\
Checks for uses of `Integer#times` that will never yield
(when the integer `<= 0`) or that will only ever yield once
(`1.times`).

# Safety

This cop is unsafe as `times` returns its receiver, which is
*usually* OK, but might change behavior.

```ruby
# bad
-5.times { do_something }
0.times { do_something }
1.times { do_something  }
1.times { |i| do_something(i) }

# good
do_something
do_something(1)
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return;
        }
        if call.name().as_slice() != b"times" {
            return;
        }
        if call.arguments().is_some() {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(int_node) = receiver.as_integer_node() else { return };
        let Ok(count): Result<i32, ()> = int_node.value().try_into() else { return };

        let mut proc_name: Option<Vec<u8>> = None;
        let mut block_node: Option<BlockNode<'_>> = None;
        if let Some(blk) = call.block() {
            if let Some(block_pass) = blk.as_block_argument_node() {
                let Some(expr) = block_pass.expression() else { return };
                let Some(sym) = expr.as_symbol_node() else { return };
                proc_name = Some(sym.unescaped().to_vec());
            } else if let Some(b) = blk.as_block_node() {
                block_node = Some(b);
            } else {
                return;
            }
        }

        if count > 1 {
            return;
        }

        let span = call.as_node().span();
        let msg = format!("Useless call to `{count}.times` detected.");

        if !ctx.begins_its_line(span) || parent_is_call_like(ctx) {
            ctx.report(&Self::META, span, msg);
            return;
        }

        let never_process = count < 1 || block_node.as_ref().is_some_and(|b| b.body().is_none());

        let edit = if never_process {
            Some(Edit::delete(ctx.whole_lines(span)))
        } else if let Some(name) = &proc_name {
            Some(Edit::replace(span, name.clone()))
        } else if let Some(block) = &block_node {
            block_replacement_edit(ctx, span, block)
        } else {
            None
        };

        match edit {
            Some(e) => ctx.report_with_fix(
                &Self::META,
                span,
                msg,
                Fix { applicability: Applicability::Unsafe, edits: vec![e] },
            ),
            None => ctx.report(&Self::META, span, msg),
        }
    }
}

/// Mirrors upstream's `node.parent&.send_type?`: true when this call is used
/// as the receiver or an argument of another call.
fn parent_is_call_like(ctx: &Context<'_>) -> bool {
    let ancestors = ctx.ancestors();
    match ancestors.last() {
        Some(info) if info.kind == NodeKind::CallNode => true,
        Some(info) if info.kind == NodeKind::ArgumentsNode => {
            ancestors.len() >= 2 && ancestors[ancestors.len() - 2].kind == NodeKind::CallNode
        }
        _ => false,
    }
}

/// Mirrors upstream's `block_arg` node matcher: the name of the block's
/// single required, non-shadowed parameter, or `None` for any other shape.
fn single_block_arg(block: &BlockNode<'_>) -> Option<Vec<u8>> {
    let params_node = block.parameters()?;
    let block_params = params_node.as_block_parameters_node()?;
    if !block_params.locals().is_empty() {
        return None;
    }
    let params = block_params.parameters()?;
    if params.requireds().len() != 1
        || !params.optionals().is_empty()
        || params.rest().is_some()
        || !params.posts().is_empty()
        || !params.keywords().is_empty()
        || params.keyword_rest().is_some()
        || params.block().is_some()
    {
        return None;
    }
    let required = params.requireds().iter().next()?;
    let required = required.as_required_parameter_node()?;
    Some(required.name().as_slice().to_vec())
}

/// Mirrors upstream's `block_reassigns_arg?`: true if `name` is reassigned
/// as a local variable anywhere within `node`.
fn block_reassigns_arg(node: &Node<'_>, name: &[u8]) -> bool {
    let mut found = false;
    each_descendant(node, &mut |n| {
        if found {
            return;
        }
        found = matches!(n.as_local_variable_write_node(), Some(w) if w.name().as_slice() == name)
            || matches!(
                n.as_local_variable_operator_write_node(),
                Some(w) if w.name().as_slice() == name
            )
            || matches!(
                n.as_local_variable_and_write_node(),
                Some(w) if w.name().as_slice() == name
            )
            || matches!(
                n.as_local_variable_or_write_node(),
                Some(w) if w.name().as_slice() == name
            )
            || matches!(
                n.as_local_variable_target_node(),
                Some(t) if t.name().as_slice() == name
            );
    });
    found
}

/// Mirrors upstream's `autocorrect_block`: replaces the whole call+block with
/// the block's (dedented, argument-substituted) body, or `None` if the block
/// reassigns its own parameter (in which case upstream leaves it uncorrected).
fn block_replacement_edit(ctx: &Context<'_>, span: Span, block: &BlockNode<'_>) -> Option<Edit> {
    let body = block.body()?;
    let block_arg = single_block_arg(block);
    if let Some(name) = &block_arg {
        if block_reassigns_arg(&block.as_node(), name) {
            return None;
        }
    }

    let body_span = body.span();
    let body_text = ctx.text(body_span);
    let substituted = match &block_arg {
        Some(name) => replace_word(body_text, name, b"0"),
        None => body_text.to_vec(),
    };

    let node_col = ctx.line_col(span.start).column;
    let body_col = ctx.line_col(body_span.start).column;
    let dedent = usize::try_from(body_col.saturating_sub(node_col)).unwrap_or(0);

    let mut lines: Vec<Vec<u8>> = substituted.split(|&b| b == b'\n').map(<[u8]>::to_vec).collect();
    for line in lines.iter_mut().skip(1) {
        if line.is_empty() {
            continue;
        }
        if line.len() >= dedent {
            line.drain(0..dedent);
        } else {
            line.clear();
        }
    }

    let mut result = Vec::with_capacity(substituted.len());
    for (idx, line) in lines.iter().enumerate() {
        if idx > 0 {
            result.push(b'\n');
        }
        result.extend_from_slice(line);
    }

    Some(Edit::replace(span, result))
}

/// Replaces whole-word occurrences of `word` with `replacement` in `text`,
/// mirroring upstream's `gsub!(/\b#{block_arg}\b/, '0')`.
fn replace_word(text: &[u8], word: &[u8], replacement: &[u8]) -> Vec<u8> {
    fn is_word_byte(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'_'
    }

    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let boundary_before = i == 0 || !is_word_byte(text[i - 1]);
        let end = i + word.len();
        let boundary_after = end >= text.len() || !is_word_byte(text[end]);
        if boundary_before && boundary_after && text[i..].starts_with(word) {
            out.extend_from_slice(replacement);
            i = end;
        } else {
            out.push(text[i]);
            i += 1;
        }
    }
    out
}
