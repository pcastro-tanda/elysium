//! `Performance/RedundantMerge`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/redundant_merge.rb`.
//!
//! whitequark's `parent`/`value_used?`/`pure?` are reimplemented over a
//! root-to-node path of Prism nodes (see [`find_path`]); whitequark elides
//! single-statement `begin`s, so a Prism `StatementsNode` is looked through.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use Hash#[]=, rather than Hash#merge! with a single key-value pair.
#[derive(Debug, Clone)]
pub struct RedundantMerge {
    max_key_value_pairs: usize,
    indentation_width: usize,
}

/// Root-to-`target` chain of nodes (inclusive); `target` is a `CallNode`
/// identified by its span.
fn find_path<'a>(node: &Node<'a>, target: Span) -> Option<Vec<Node<'a>>> {
    let span = node.span();
    if span.start == target.start && span.end == target.end && node.kind() == NodeKind::CallNode {
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

fn same_span(a: &Node<'_>, b: &Node<'_>) -> bool {
    let (a, b) = (a.span(), b.span());
    a.start == b.start && a.end == b.end
}

/// rubocop-ast's `Node#value_used?` for `path[idx]`.
fn value_used(path: &[Node<'_>], idx: usize) -> bool {
    let Some(parent_idx) = idx.checked_sub(1) else { return false };
    let node = &path[idx];
    let parent = &path[parent_idx];
    match parent.kind() {
        NodeKind::ProgramNode => false,
        NodeKind::StatementsNode => {
            let statements = parent.as_statements_node().expect("kind matched");
            let is_last = statements.body().iter().last().is_some_and(|l| same_span(&l, node));
            is_last && value_used(path, parent_idx)
        }
        NodeKind::BeginNode => {
            let begin = parent.as_begin_node().expect("kind matched");
            if begin.rescue_clause().is_some()
                || begin.else_clause().is_some()
                || begin.ensure_clause().is_some()
            {
                true
            } else {
                value_used(path, parent_idx)
            }
        }
        NodeKind::IfNode => {
            let pred = parent.as_if_node().expect("kind matched").predicate();
            same_span(&pred, node) || value_used(path, parent_idx)
        }
        NodeKind::UnlessNode => {
            let pred = parent.as_unless_node().expect("kind matched").predicate();
            same_span(&pred, node) || value_used(path, parent_idx)
        }
        NodeKind::CaseNode => {
            let pred = parent.as_case_node().expect("kind matched").predicate();
            pred.is_some_and(|p| same_span(&p, node)) || value_used(path, parent_idx)
        }
        NodeKind::ParenthesesNode
        | NodeKind::EmbeddedStatementsNode
        | NodeKind::ElseNode
        | NodeKind::WhenNode
        | NodeKind::ArrayNode
        | NodeKind::DefinedNode
        | NodeKind::InterpolatedStringNode
        | NodeKind::InterpolatedSymbolNode
        | NodeKind::InterpolatedRegularExpressionNode
        | NodeKind::InterpolatedXStringNode
        | NodeKind::FlipFlopNode
        | NodeKind::RangeNode
        | NodeKind::HashNode
        | NodeKind::AssocNode => value_used(path, parent_idx),
        NodeKind::WhileNode => {
            let pred = parent.as_while_node().expect("kind matched").predicate();
            same_span(&pred, node)
        }
        NodeKind::UntilNode => {
            let pred = parent.as_until_node().expect("kind matched").predicate();
            same_span(&pred, node)
        }
        NodeKind::ForNode => {
            let stmts = parent.as_for_node().expect("kind matched").statements();
            if stmts.is_some_and(|s| same_span(&s.as_node(), node)) {
                value_used(path, parent_idx)
            } else {
                true
            }
        }
        _ => true,
    }
}

/// rubocop-ast's `Node#pure?`.
fn is_pure(node: &Node<'_>) -> bool {
    let all = |node: &Node<'_>| {
        let mut ok = true;
        for_each_child(node, |c| ok = ok && is_pure(c));
        ok
    };
    match node.kind() {
        NodeKind::SourceFileNode
        | NodeKind::SourceLineNode
        | NodeKind::ConstantReadNode
        | NodeKind::ConstantPathNode
        | NodeKind::ClassVariableReadNode
        | NodeKind::DefinedNode
        | NodeKind::FalseNode
        | NodeKind::FloatNode
        | NodeKind::GlobalVariableReadNode
        | NodeKind::IntegerNode
        | NodeKind::InstanceVariableReadNode
        | NodeKind::LocalVariableReadNode
        | NodeKind::NilNode
        | NodeKind::StringNode
        | NodeKind::SymbolNode
        | NodeKind::TrueNode => true,
        NodeKind::AndNode
        | NodeKind::OrNode
        | NodeKind::ArrayNode
        | NodeKind::ParenthesesNode
        | NodeKind::StatementsNode
        | NodeKind::CaseNode
        | NodeKind::WhenNode
        | NodeKind::InterpolatedStringNode
        | NodeKind::InterpolatedSymbolNode
        | NodeKind::InterpolatedRegularExpressionNode
        | NodeKind::EmbeddedStatementsNode
        | NodeKind::EmbeddedVariableNode
        | NodeKind::FlipFlopNode
        | NodeKind::RangeNode
        | NodeKind::HashNode
        | NodeKind::AssocNode
        | NodeKind::IfNode
        | NodeKind::UnlessNode
        | NodeKind::ElseNode
        | NodeKind::WhileNode
        | NodeKind::UntilNode
        | NodeKind::EnsureNode => all(node),
        NodeKind::BeginNode => {
            let begin = node.as_begin_node().expect("kind matched");
            begin.rescue_clause().is_none() && begin.else_clause().is_none() && all(node)
        }
        _ => false,
    }
}

/// `unwind(receiver)` followed by the `lvar` check; returns the variable name.
fn unwound_lvar_name(receiver: &Node<'_>) -> Option<Vec<u8>> {
    let mut current = *receiver;
    while let Some(call) = current.as_call_node() {
        if call.is_safe_navigation() || call.block().is_some_and(|b| b.as_block_node().is_some()) {
            return None;
        }
        current = call.receiver()?;
    }
    current.as_local_variable_read_node().map(|n| n.name().as_slice().to_vec())
}

/// `second_argument` of `EachWithObjectInspector`: the name of the block's
/// second parameter when the merge is directly inside an `each_with_object`
/// block with exactly two parameters.
fn each_with_object_second_arg(path: &[Node<'_>], idx: usize) -> Option<Vec<u8>> {
    let mut j = idx.checked_sub(1)?;
    if path[j].kind() == NodeKind::StatementsNode {
        j = j.checked_sub(1)?;
    }
    if path[j].kind() == NodeKind::ParenthesesNode {
        j = j.checked_sub(1)?;
        if path[j].kind() == NodeKind::StatementsNode {
            j = j.checked_sub(1)?;
        }
    }
    let block = path[j].as_block_node()?;
    let owner = path[j.checked_sub(1)?].as_call_node()?;
    if owner.name().as_slice() != b"each_with_object" {
        return None;
    }
    let mut args = owner.arguments()?.arguments().iter();
    if args.next().is_none() || args.next().is_some() {
        return None;
    }
    let params_node = block.parameters()?;
    let params = params_node.as_block_parameters_node()?.parameters()?;
    let mut all: Vec<Node<'_>> = Vec::new();
    all.extend(params.requireds().iter());
    all.extend(params.optionals().iter());
    if let Some(rest) = params.rest() {
        all.push(rest);
    }
    all.extend(params.posts().iter());
    all.extend(params.keywords().iter());
    if let Some(kr) = params.keyword_rest() {
        all.push(kr);
    }
    if let Some(b) = params.block() {
        all.push(b.as_node());
    }
    if all.len() != 2 {
        return None;
    }
    let second = &all[1];
    if let Some(n) = second.as_required_parameter_node() {
        Some(n.name().as_slice().to_vec())
    } else if let Some(n) = second.as_optional_parameter_node() {
        Some(n.name().as_slice().to_vec())
    } else if let Some(n) = second.as_rest_parameter_node() {
        n.name().map(|c| c.as_slice().to_vec())
    } else if let Some(n) = second.as_keyword_rest_parameter_node() {
        n.name().map(|c| c.as_slice().to_vec())
    } else if let Some(n) = second.as_block_parameter_node() {
        n.name().map(|c| c.as_slice().to_vec())
    } else if let Some(n) = second.as_required_keyword_parameter_node() {
        Some(n.name().as_slice().to_vec())
    } else {
        second.as_optional_keyword_parameter_node().map(|n| n.name().as_slice().to_vec())
    }
}

/// Source of a pair's key as `to_assignments` renders it.
fn key_source(ctx: &Context<'_>, key: &Node<'_>) -> Vec<u8> {
    let span = key.span();
    let text = ctx.text(span);
    let closing = key
        .as_symbol_node()
        .and_then(|s| s.closing_loc())
        .or_else(|| key.as_interpolated_symbol_node().and_then(|s| s.closing_loc()));
    let colon_style = closing.is_some_and(|c| ctx.text(c.span()).ends_with(b":"));
    if colon_style {
        let stripped = &text[..text.len() - 1];
        if key.kind() == NodeKind::SymbolNode {
            let mut out = vec![b':'];
            out.extend_from_slice(stripped);
            return out;
        }
        return stripped.to_vec();
    }
    text.to_vec()
}

fn value_source(ctx: &Context<'_>, key: &Node<'_>, value: &Node<'_>) -> Vec<u8> {
    if value.kind() == NodeKind::ImplicitNode {
        // `{id:}` -- the value's source is the key's name without the colon.
        let text = ctx.text(key.span());
        return text[..text.len().saturating_sub(1)].to_vec();
    }
    ctx.text(value.span()).to_vec()
}

fn leading_spaces(ctx: &Context<'_>, offset: u32) -> Vec<u8> {
    let line = ctx.line_col(offset).line;
    ctx.line_text(line).iter().take_while(|b| b.is_ascii_whitespace()).copied().collect()
}

fn gsub_newlines(source: &[u8], padding: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(source.len());
    for &b in source {
        if b == b'\n' {
            out.extend_from_slice(padding);
        } else {
            out.push(b);
        }
    }
    out
}

impl Rule for RedundantMerge {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RedundantMerge",
        department: Department::Performance,
        summary: "Use Hash#[]=, rather than Hash#merge! with a single key-value pair.",
        explanation: "\
Identifies places where `Hash#merge!` can be replaced by `Hash#[]=`.
You can set the maximum number of key-value pairs to consider
an offense with `MaxKeyValuePairs`.

This cop is unsafe because RuboCop cannot determine if the
receiver of `merge!` is actually a hash or not.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "MaxKeyValuePairs",
            default: ConfigDefault::Int(2),
            allowed: &[],
            doc: "Max number of key-value pairs to consider an offense.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let max = usize::try_from(options.int("MaxKeyValuePairs")).unwrap_or(2);
        let width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(linter::OptionValue::as_int)
            .and_then(|w| usize::try_from(w).ok())
            .unwrap_or(2);
        Ok(Self { max_key_value_pairs: max, indentation_width: width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() || call.name().as_slice() != b"merge!" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        // A `BlockArgumentNode` is a third argument in whitequark; with a literal
        // block the send's parent is the block, so its value counts as used.
        if call.block().is_some() {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut it = arguments.arguments().iter();
        let (Some(arg), None) = (it.next(), it.next()) else { return };
        let pairs: Vec<Node<'_>> = if let Some(h) = arg.as_hash_node() {
            h.elements().iter().collect()
        } else if let Some(h) = arg.as_keyword_hash_node() {
            h.elements().iter().collect()
        } else {
            return;
        };

        // non_redundant_merge?
        if pairs.is_empty()
            || (pairs.len() > 1 && !is_pure(&receiver))
            || pairs.len() > self.max_key_value_pairs
            || pairs.iter().any(|p| p.kind() == NodeKind::AssocSplatNode)
        {
            return;
        }
        let node_span = node.span();
        let Some(path) = find_path(&ctx.parsed().root(), node.span()) else { return };
        let idx = path.len() - 1;
        if value_used(&path, idx) {
            // EachWithObjectInspector#value_used?
            let receiver_name = unwound_lvar_name(&receiver);
            let inspected = match (receiver_name, each_with_object_second_arg(&path, idx)) {
                (Some(r), Some(s)) => r == s,
                _ => false,
            };
            if !inspected {
                return;
            }
        }

        let receiver_source = ctx.text(receiver.span()).to_vec();
        let mut assignments: Vec<Vec<u8>> = Vec::new();
        for pair in &pairs {
            let Some(assoc) = pair.as_assoc_node() else { return };
            let key = assoc.key();
            let mut line = receiver_source.clone();
            line.push(b'[');
            line.extend(key_source(ctx, &key));
            line.extend_from_slice(b"] = ");
            line.extend(value_source(ctx, &key, &assoc.value()));
            assignments.push(line);
        }
        let prefer = String::from_utf8_lossy(&assignments.join(&b"; "[..])).into_owned();
        let current = String::from_utf8_lossy(ctx.text(node_span)).into_owned();
        let message = format!("Use `{prefer}` instead of `{current}`.");

        let new_source = assignments.join(&b"\n"[..]);
        // whitequark's `node.parent` is nil only for a lone top-level statement.
        let parent_idx = idx - 1;
        let wq_parent_idx = if path[parent_idx].kind() == NodeKind::StatementsNode {
            let single =
                path[parent_idx].as_statements_node().is_some_and(|s| s.body().iter().count() == 1);
            if single {
                parent_idx.checked_sub(1).filter(|&i| path[i].kind() != NodeKind::ProgramNode)
            } else {
                Some(parent_idx)
            }
        } else {
            Some(parent_idx)
        };

        let edit = match wq_parent_idx {
            Some(pi) if pairs.len() > 1 => {
                let parent = &path[pi];
                let modifier = modifier_flow_control(ctx, parent);
                if let Some((keyword, condition)) = modifier {
                    let indent = vec![b' '; self.indentation_width];
                    let spaces = leading_spaces(ctx, node.span().start);
                    let mut padding = vec![b'\n'];
                    padding.extend_from_slice(&indent);
                    padding.extend_from_slice(&spaces);
                    let body = gsub_newlines(&new_source, &padding);
                    let mut out = keyword;
                    out.push(b' ');
                    out.extend(condition);
                    out.push(b'\n');
                    out.extend_from_slice(&spaces);
                    out.extend_from_slice(&indent);
                    out.extend(body);
                    out.push(b'\n');
                    out.extend_from_slice(&spaces);
                    out.extend_from_slice(b"end");
                    Edit::replace(parent.span(), out)
                } else {
                    let mut padding = vec![b'\n'];
                    padding.extend(leading_spaces(ctx, node.span().start));
                    Edit::replace(node_span, gsub_newlines(&new_source, &padding))
                }
            }
            _ => Edit::replace(node_span, new_source),
        };
        let fix = Fix { applicability: Applicability::Unsafe, edits: vec![edit] };
        ctx.report_with_fix(&Self::META, node_span, message, fix);
    }
}

/// `modifier_flow_control?(parent)`: the keyword and condition source when
/// `parent` is a modifier `if`/`unless`/`while`/`until`.
fn modifier_flow_control(ctx: &Context<'_>, parent: &Node<'_>) -> Option<(Vec<u8>, Vec<u8>)> {
    let text = |n: &Node<'_>| ctx.text(n.span()).to_vec();
    let kw = |s: Span| ctx.text(s).to_vec();
    match parent.kind() {
        NodeKind::IfNode => {
            let n = parent.as_if_node()?;
            let keyword = n.if_keyword_loc()?.span();
            (n.end_keyword_loc().is_none() && keyword.start > parent.span().start)
                .then(|| (kw(keyword), text(&n.predicate())))
        }
        NodeKind::UnlessNode => {
            let n = parent.as_unless_node()?;
            let keyword = n.keyword_loc().span();
            (n.end_keyword_loc().is_none() && keyword.start > parent.span().start)
                .then(|| (kw(keyword), text(&n.predicate())))
        }
        NodeKind::WhileNode => {
            let n = parent.as_while_node()?;
            let keyword = n.keyword_loc().span();
            (n.closing_loc().is_none() && keyword.start > parent.span().start)
                .then(|| (kw(keyword), text(&n.predicate())))
        }
        NodeKind::UntilNode => {
            let n = parent.as_until_node()?;
            let keyword = n.keyword_loc().span();
            (n.closing_loc().is_none() && keyword.start > parent.span().start)
                .then(|| (kw(keyword), text(&n.predicate())))
        }
        _ => None,
    }
}
