//! `Sorbet/BlockMethodDefinition`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/block_method_definition.rb`.

use std::collections::HashSet;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{each_descendant, for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Do not define methods in blocks (use `define_method` as a workaround).";

/// Disallows defining methods inside blocks without using `define_method`, unless the block is a named class definition. This is to avoid running into <https://github.com/sorbet/sorbet/issues/3609>.
#[derive(Debug, Clone)]
pub struct BlockMethodDefinition {
    /// RuboCop drops an offense whose range was already reported.
    seen: HashSet<(u32, u32)>,
}

impl Rule for BlockMethodDefinition {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/BlockMethodDefinition",
        department: Department::Sorbet,
        summary: "Disallows defining methods inside blocks without using `define_method`, unless the block is a named class definition. This is to avoid running into https://github.com/sorbet/sorbet/issues/3609.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode, NodeKind::LambdaNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { seen: HashSet::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        // on_block / on_numblock / on_itblock (a lambda literal is a `block`).
        if let Some(call) = node.as_call_node() {
            if call.block().is_none_or(|b| b.as_block_node().is_none()) {
                return;
            }
        } else if node.as_lambda_node().is_none() {
            return;
        }
        if ctx.parent().is_some_and(|p| {
            matches!(p.kind, NodeKind::ConstantWriteNode | NodeKind::ConstantPathWriteNode)
        }) {
            return;
        }
        if in_activesupport_concern_class_methods_block(node, ctx) {
            return;
        }
        let mut defs: Vec<Node<'_>> = Vec::new();
        each_descendant(node, &mut |d| {
            if d.as_def_node().is_some() {
                defs.push(*d);
            }
        });
        for def in defs {
            let span = def.span();
            if !self.seen.insert((span.start, span.end)) {
                continue;
            }
            let edit = autocorrect(&def, ctx);
            ctx.report_with_fix(
                &Self::META,
                span,
                MSG,
                Fix { applicability: Applicability::Unsafe, edits: vec![edit] },
            );
        }
    }
}

/// `(const {nil? cbase} :Name)` for a single constant name.
fn const_in_root(node: &Node<'_>, name: &[u8]) -> bool {
    if let Some(c) = node.as_constant_read_node() {
        return c.name().as_slice() == name;
    }
    node.as_constant_path_node()
        .is_some_and(|p| p.parent().is_none() && p.name().is_some_and(|n| n.as_slice() == name))
}

/// `(const (const {nil? cbase} :ActiveSupport) :Concern)`.
fn active_support_concern(node: &Node<'_>) -> bool {
    node.as_constant_path_node().is_some_and(|p| {
        p.name().is_some_and(|n| n.as_slice() == b"Concern")
            && p.parent().is_some_and(|parent| const_in_root(&parent, b"ActiveSupport"))
    })
}

fn find_node<'a>(node: &Node<'a>, span: Span, kind: NodeKind) -> Option<Node<'a>> {
    let s = node.span();
    if s.start == span.start && s.end == span.end && node.kind() == kind {
        return Some(*node);
    }
    let mut found = None;
    for_each_child(node, |child| {
        if found.is_none() {
            let cs = child.span();
            if cs.start <= span.start && span.end <= cs.end {
                found = find_node(child, span, kind);
            }
        }
    });
    found
}

fn module_extends_activesupport_concern(module: &Node<'_>) -> bool {
    let Some(module) = module.as_module_node() else { return false };
    let Some(body) = module.body() else { return false };
    let Some(statements) = body.as_statements_node() else { return false };
    let list = statements.body();
    if list.iter().count() < 2 {
        return false;
    }
    list.iter().any(|stmt| {
        let Some(call) = stmt.as_call_node() else { return false };
        if call.is_safe_navigation()
            || call.receiver().is_some()
            || call.name().as_slice() != b"extend"
            || call.block().is_some()
        {
            return false;
        }
        let Some(args) = call.arguments() else { return false };
        let mut it = args.arguments().iter();
        matches!((it.next(), it.next()), (Some(a), None) if active_support_concern(&a))
    })
}

fn in_activesupport_concern_class_methods_block(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.is_safe_navigation()
        || call.receiver().is_some()
        || call.arguments().is_some()
        || call.name().as_slice() != b"class_methods"
    {
        return false;
    }
    let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return false };
    if let Some(params) = block.parameters() {
        if params.as_block_parameters_node().is_none() {
            return false;
        }
    }
    let Some(info) = ctx.ancestors().iter().rev().find(|a| a.kind == NodeKind::ModuleNode) else {
        return false;
    };
    let root = ctx.parsed().root();
    find_node(&root, info.span, NodeKind::ModuleNode)
        .is_some_and(|m| module_extends_activesupport_concern(&m))
}

fn def_arguments<'a>(def: &ruby_ast::node::DefNode<'a>) -> Vec<Node<'a>> {
    let Some(params) = def.parameters() else { return Vec::new() };
    let mut out: Vec<Node<'a>> = Vec::new();
    out.extend(params.requireds().iter());
    out.extend(params.optionals().iter());
    out.extend(params.rest());
    out.extend(params.posts().iter());
    out.extend(params.keywords().iter());
    out.extend(params.keyword_rest());
    out.extend(params.block().map(|b| b.as_node()));
    out.sort_by_key(|n| n.span().start);
    out
}

fn autocorrect(node: &Node<'_>, ctx: &Context<'_>) -> Edit {
    let def = node.as_def_node().expect("def node");
    let span = node.span();
    let indent_width = ctx.line_col(span.start).column as usize;
    let indent = " ".repeat(indent_width);

    let method_name = String::from_utf8_lossy(def.name().as_slice()).into_owned();
    let args = def_arguments(&def);
    let args_str = if args.is_empty() {
        String::new()
    } else {
        let joined: Vec<String> =
            args.iter().map(|a| String::from_utf8_lossy(ctx.text(a.span())).into_owned()).collect();
        format!(" |{}|", joined.join(", "))
    };

    let signature_replacement = match def.receiver() {
        None => format!("define_method(:{method_name}) do{args_str}"),
        Some(receiver) => format!(
            "{}.define_singleton_method(:{method_name}) do{args_str}",
            String::from_utf8_lossy(ctx.text(receiver.span()))
        ),
    };

    let (end_pos, indentation) = if let Some(body) = def.body() {
        (body.span().start, format!("\n{indent}  "))
    } else if !ctx.text(span).contains(&b'\n') {
        (span.end, format!("\n{indent}end"))
    } else if let Some(last) = args.last() {
        let end = last.span().end;
        let rest = &ctx.source().bytes()[end as usize..];
        let ws = rest.iter().take_while(|b| b.is_ascii_whitespace()).count();
        let end_pos = if rest.get(ws) == Some(&b')') {
            end + u32::try_from(ws + 1).expect("offset")
        } else {
            end
        };
        (end_pos, String::new())
    } else {
        (def.name_loc().span().end, String::new())
    };

    Edit::replace(
        Span::new(span.start, end_pos),
        format!("{signature_replacement}{indentation}").into_bytes(),
    )
}
