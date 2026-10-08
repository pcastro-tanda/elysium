//! `Sorbet/SetterReturnType`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/setter_return_type.rb`.

use linter::{
    Applicability, CommentKind, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{for_each_child, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Setter methods must declare a `void` return type.";

const NON_SETTER_OPERATORS: [&[u8]; 5] = [b"==", b"===", b"!=", b"<=", b">="];

/// Checks that setter methods declare a `void` return type in their signature.
#[derive(Debug, Clone)]
pub struct SetterReturnType;

impl Rule for SetterReturnType {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/SetterReturnType",
        department: Department::Sorbet,
        summary: "Checks that setter methods declare a `void` return type in their signature.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        let name = def.name();
        let name = name.as_slice();
        if !name.ends_with(b"=") || NON_SETTER_OPERATORS.contains(&name) {
            return;
        }
        let root = ctx.parsed().root();
        let Some(path) = find_path(&root, node.span()) else { return };

        // outermost_send_ancestor
        let mut target = path.len() - 1;
        while target > 0 {
            let parent = &path[target - 1];
            let child_span = path[target].span();
            if let Some(call) = parent.as_call_node() {
                if !call.is_safe_navigation()
                    && call.receiver().is_some_and(|r| same_span(&r, child_span))
                {
                    target -= 1;
                    continue;
                }
            } else if parent.as_arguments_node().is_some()
                && target >= 2
                && path[target - 2].as_call_node().is_some_and(|c| !c.is_safe_navigation())
            {
                target -= 2;
                continue;
            }
            break;
        }

        // A call carrying a literal block is wrapped in a `block` node in
        // whitequark: the send is then neither a statement sibling nor the
        // node comments are associated with.
        let wrapped_by_block = path[target]
            .as_call_node()
            .is_some_and(|c| c.block().is_some_and(|b| b.as_block_node().is_some()));

        let sigs = if wrapped_by_block { Vec::new() } else { preceding_sigs(&path, target) };
        if !sigs.is_empty() {
            for sig in &sigs {
                check_sig(sig, ctx);
            }
        } else if !wrapped_by_block {
            check_rbs(&path, target, ctx);
        }
    }
}

fn same_span(node: &Node<'_>, span: Span) -> bool {
    let s = node.span();
    s.start == span.start && s.end == span.end
}

/// Root-to-def chain of nodes (inclusive).
fn find_path<'a>(node: &Node<'a>, target: Span) -> Option<Vec<Node<'a>>> {
    if node.kind() == NodeKind::DefNode && same_span(node, target) {
        return Some(vec![*node]);
    }
    let mut children = Vec::new();
    for_each_child(node, |child| {
        let cs = child.span();
        if cs.start <= target.start && target.end <= cs.end {
            children.push(*child);
        }
    });
    for child in &children {
        if let Some(mut path) = find_path(child, target) {
            path.insert(0, *node);
            return Some(path);
        }
    }
    None
}

/// The `sig` blocks immediately preceding `target` among its siblings,
/// supporting consecutive overload sigs. The scan stops at the first
/// non-signature sibling.
fn preceding_sigs<'a>(path: &[Node<'a>], target: usize) -> Vec<Node<'a>> {
    if target == 0 {
        return Vec::new();
    }
    let Some(statements) = path[target - 1].as_statements_node() else { return Vec::new() };
    let siblings: Vec<Node<'a>> = statements.body().iter().collect();
    let Some(index) = siblings.iter().position(|s| same_span(s, path[target].span())) else {
        return Vec::new();
    };
    let mut sigs = Vec::new();
    for sibling in siblings[..index].iter().rev() {
        if !is_signature(sibling) {
            break;
        }
        sigs.insert(0, *sibling);
    }
    sigs
}

fn check_sig(sig: &Node<'_>, ctx: &mut Context<'_>) {
    let Some(block) = sig.as_call_node().and_then(|c| c.block()).and_then(|b| b.as_block_node())
    else {
        return;
    };
    let body = match block.body() {
        Some(body) => match body.as_statements_node() {
            Some(statements) => {
                let mut it = statements.body().iter();
                match (it.next(), it.next()) {
                    (Some(only), None) => Some(only),
                    _ => None,
                }
            }
            None => None,
        },
        None => None,
    };
    let Some(decl) = body.and_then(|b| return_declaration(&b)) else { return };
    let Some(call) = decl.as_call_node() else { return };
    if call.name().as_slice() == b"void" {
        return;
    }
    let last_argument = call
        .block()
        .map(|b| b.span())
        .or_else(|| call.arguments().and_then(|a| a.arguments().iter().last().map(|n| n.span())));
    let has_argument = call.arguments().is_some_and(|a| a.arguments().iter().next().is_some())
        || call.block().is_some();
    if !has_argument {
        return; // malformed returns
    }
    let Some(selector) = call.message_loc() else { return };
    let start = selector.start_offset();
    let end = call
        .closing_loc()
        .map(|l| l.end_offset())
        .or_else(|| last_argument.map(|s| s.end as usize))
        .unwrap_or_else(|| selector.end_offset());
    let (Ok(start), Ok(end)) = (u32::try_from(start), u32::try_from(end)) else { return };
    let range = Span::new(start, end);
    ctx.report_with_fix(
        &SetterReturnType::META,
        range,
        MSG,
        Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(range, b"void".to_vec())],
        },
    );
}

/// Walks the receiver chain of the sig body and returns the outermost `void`
/// or `returns` node.
fn return_declaration<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() || call.block().is_some_and(|b| b.as_block_node().is_some()) {
        return None;
    }
    if matches!(call.name().as_slice(), b"void" | b"returns") {
        Some(*node)
    } else {
        return_declaration(&call.receiver()?)
    }
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0c | 0x0b)
}

fn check_rbs(path: &[Node<'_>], target: usize, ctx: &mut Context<'_>) {
    let target_node = path[target];
    let target_start = target_node.span().start;
    // Comments are associated with the outermost node starting after them;
    // `begin` (statement lists) never takes them.
    let shadowed = path[..target].iter().any(|a| {
        !matches!(
            a.kind(),
            NodeKind::StatementsNode | NodeKind::ArgumentsNode | NodeKind::ProgramNode
        ) && a.span().start == target_start
    });
    if shadowed {
        return;
    }
    let target_line = ctx.line_col(target_start).line;

    // comments_above: the contiguous own-line comment block right above.
    let mut block: Vec<Span> = Vec::new();
    let mut expected = target_line;
    let candidates: Vec<(u32, Span)> = ctx
        .comments()
        .iter()
        .filter(|c| c.kind == CommentKind::Inline)
        .filter_map(|c| {
            let line = ctx.line_col(c.span.start).line;
            let line_start = ctx.line_span(line).start;
            let own_line = ctx.text(Span::new(line_start, c.span.start)).iter().all(|&b| is_ws(b));
            (line < target_line && own_line).then_some((line, c.span))
        })
        .collect();
    for &(line, span) in candidates.iter().rev() {
        if line + 1 != expected {
            break;
        }
        block.insert(0, span);
        expected = line;
    }

    let mut groups: Vec<Vec<Span>> = Vec::new();
    for span in block {
        let text = ctx.text(span);
        if text.starts_with(b"#:") {
            groups.push(vec![span]);
        } else if text.starts_with(b"#|") {
            if let Some(last) = groups.last_mut() {
                last.push(span);
            }
        }
    }

    for group in groups {
        let Some((highlight, replace, void)) = analyze_signature(&group, ctx) else { continue };
        if void {
            continue;
        }
        ctx.report_with_fix(
            &SetterReturnType::META,
            highlight,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(replace, b"void".to_vec())],
            },
        );
    }
}

/// `[highlight_range, replace_range, void?]` for one RBS signature, or
/// `None` without a return type.
fn analyze_signature(group: &[Span], ctx: &Context<'_>) -> Option<(Span, Span, bool)> {
    // strip_rbs_prefix
    let segments: Vec<(&[u8], usize)> = group
        .iter()
        .map(|&span| {
            let text = ctx.text(span);
            if text.len() >= 2 && text[0] == b'#' && (text[1] == b':' || text[1] == b'|') {
                let ws = text[2..].iter().take_while(|&&b| is_ws(b)).count();
                (&text[2 + ws..], 2 + ws)
            } else {
                (text, 0)
            }
        })
        .collect();
    let joined: Vec<u8> = segments.iter().map(|s| s.0).collect::<Vec<_>>().join(&b' ');

    // method_arrow_index
    let mut depth = 0i32;
    let mut arrow = None;
    for (index, &byte) in joined.iter().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'-' if depth == 0 && joined.get(index + 1) == Some(&b'>') => {
                arrow = Some(index);
                break;
            }
            _ => {}
        }
    }
    let arrow = arrow?;

    let after = &joined[arrow + 2..];
    let lead = after.iter().take_while(|&&b| is_ws(b)).count();
    let expr = after[lead..].trim_ascii_end();
    if expr.is_empty() {
        return None;
    }
    let token_len = expr.iter().take_while(|&&b| !is_ws(b)).count();
    let token_start = arrow + 2 + lead;

    // range_for_token
    let mut offset = 0usize;
    let mut highlight = None;
    for (index, &(content, prefix_len)) in segments.iter().enumerate() {
        let seg_end = offset + content.len();
        if token_start < seg_end {
            let start = group[index].start as usize + prefix_len + (token_start - offset);
            highlight = Some((start, start + token_len));
            break;
        }
        offset = seg_end + 1;
    }
    let (hl_start, hl_end) = highlight?;

    // return_end_pos
    let (last_content, last_prefix) = *segments.last()?;
    let last_trimmed = last_content.trim_ascii_end().len();
    let replace_end = group.last()?.start as usize + last_prefix + last_trimmed;
    if replace_end < hl_start {
        return None;
    }
    let to_u32 = |v: usize| u32::try_from(v).ok();
    Some((
        Span::new(to_u32(hl_start)?, to_u32(hl_end)?),
        Span::new(to_u32(hl_start)?, to_u32(replace_end)?),
        expr == b"void",
    ))
}

/// `(const {nil? cbase} :T)`.
fn is_t_const(node: &Node<'_>) -> bool {
    if let Some(c) = node.as_constant_read_node() {
        return c.name().as_slice() == b"T";
    }
    node.as_constant_path_node()
        .is_some_and(|p| p.parent().is_none() && p.name().is_some_and(|n| n.as_slice() == b"T"))
}

/// `(const (const {nil? cbase} :T) :Sig)`.
fn is_t_sig_const(node: &Node<'_>) -> bool {
    node.as_constant_path_node().is_some_and(|p| {
        p.name().is_some_and(|n| n.as_slice() == b"Sig")
            && p.parent().is_some_and(|parent| is_t_const(&parent))
    })
}

/// `(const (const (const {nil? cbase} :T) :Sig) :WithoutRuntime)`.
fn is_t_sig_without_runtime_const(node: &Node<'_>) -> bool {
    node.as_constant_path_node().is_some_and(|p| {
        p.name().is_some_and(|n| n.as_slice() == b"WithoutRuntime")
            && p.parent().is_some_and(|parent| is_t_sig_const(&parent))
    })
}

/// `SignatureHelp`'s `signature?`: `bare_sig?`, `sig_with_runtime?` or
/// `sig_without_runtime?` -- a `block` (not numblock/itblock) with empty
/// `(args)` around `sig` / `sig(:final)`.
fn is_signature(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.is_safe_navigation() || call.name().as_slice() != b"sig" {
        return false;
    }
    let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return false };
    if let Some(params) = block.parameters() {
        let Some(params) = params.as_block_parameters_node() else { return false };
        if params.parameters().is_some() || params.locals().iter().next().is_some() {
            return false;
        }
    }
    if let Some(arguments) = call.arguments() {
        let mut it = arguments.arguments().iter();
        let ok = matches!((it.next(), it.next()), (Some(a), None)
            if a.as_symbol_node().is_some_and(|s| s.unescaped() == b"final"));
        if !ok {
            return false;
        }
    }
    match call.receiver() {
        None => true,
        Some(receiver) => is_t_sig_const(&receiver) || is_t_sig_without_runtime_const(&receiver),
    }
}
