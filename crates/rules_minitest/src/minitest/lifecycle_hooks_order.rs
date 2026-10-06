//! `Minitest/LifecycleHooksOrder`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/lifecycle_hooks_order.rb` (with its
//! `MinitestExplorationHelpers` mixin and rubocop's `DefNode`,
//! `VisibilityHelp` and `RangeHelp#range_with_comments_and_lines`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, ClassNode, DefNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const LIFECYCLE_HOOK_METHODS_IN_ORDER: &[&[u8]] = &[
    b"before_setup",
    b"setup",
    b"after_setup",
    b"before_teardown",
    b"teardown",
    b"after_teardown",
];

const VISIBILITY_SCOPES: &[&[u8]] = &[b"private", b"protected", b"public"];

/// Checks that lifecycle hooks are declared in the order in which they will be executed.
#[derive(Debug, Clone)]
pub struct LifecycleHooksOrder;

impl Rule for LifecycleHooksOrder {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/LifecycleHooksOrder",
        department: Department::Minitest,
        summary:
            "Checks that lifecycle hooks are declared in the order in which they will be executed.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ClassNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(class) = node.as_class_node() else { return };
        if !test_class(&class, ctx) {
            return;
        }

        let mut previous_index: usize = 0;
        let mut previous: Option<ClassDef<'_>> = None;
        let mut first = true;

        for entry in class_def_nodes(&class)
            .into_iter()
            .filter(|entry| lifecycle_hook_method(&entry.def) || test_method(entry))
        {
            let hook = entry.def.name();
            let hook = hook.as_slice();
            let index = hooks_order(hook);

            if let (false, Some(previous_entry)) = (first, previous.as_ref()) {
                if index < previous_index {
                    let message = format!(
                        "`{}` is supposed to appear before `{}`.",
                        String::from_utf8_lossy(hook),
                        String::from_utf8_lossy(previous_entry.def.name().as_slice()),
                    );
                    let previous_range = range_with_comments_and_lines(
                        ctx,
                        previous_entry.def.location().span(),
                        previous_entry.prev_end,
                    );
                    let node_range = range_with_comments_and_lines(
                        ctx,
                        entry.def.location().span(),
                        entry.prev_end,
                    );
                    let source = ctx.text(node_range).to_vec();
                    ctx.report_with_fix(
                        &Self::META,
                        entry.def.location().span(),
                        message,
                        Fix {
                            applicability: Applicability::Safe,
                            edits: vec![
                                Edit::insert(previous_range.start, source),
                                Edit::delete(node_range),
                            ],
                        },
                    );
                }
            }
            first = false;
            previous_index = index;
            previous = Some(entry);
        }
    }
}

/// `HOOKS_ORDER_MAP`: a hook's position, or one past the last for any other method.
fn hooks_order(name: &[u8]) -> usize {
    LIFECYCLE_HOOK_METHODS_IN_ORDER
        .iter()
        .position(|hook| *hook == name)
        .unwrap_or(LIFECYCLE_HOOK_METHODS_IN_ORDER.len() + 1)
}

/// `class_node.parent_class && class_node.identifier.source.end_with?('Test')`.
fn test_class(class: &ClassNode<'_>, ctx: &Context<'_>) -> bool {
    class.superclass().is_some() && ctx.text(class.constant_path().span()).ends_with(b"Test")
}

fn lifecycle_hook_method(def: &DefNode<'_>) -> bool {
    LIFECYCLE_HOOK_METHODS_IN_ORDER.contains(&def.name().as_slice())
}

/// `test_case?` for a `def` found by `class_def_nodes`:
/// `test_method?(node)` (visibility check on) within a test class.
fn test_method(entry: &ClassDef<'_>) -> bool {
    !entry.non_public()
        && entry.def.name().as_slice().starts_with(b"test_")
        && entry.def.parameters().is_none()
}

/// A `def` child of a class body with what `VisibilityHelp` and `RangeHelp`
/// need to know about its surroundings.
struct ClassDef<'pr> {
    def: DefNode<'pr>,
    /// `node.left_siblings`, in source order.
    before: Vec<Node<'pr>>,
    /// `node.right_siblings`, in source order.
    after: Vec<Node<'pr>>,
    /// End of whatever the comment associator visits just before the `def`
    /// (the previous statement, else the superclass).
    prev_end: u32,
    /// The name of a `private def foo`-style call the `def` is the argument of.
    parent_modifier: Option<Vec<u8>>,
}

impl ClassDef<'_> {
    /// `non_public?`: `non_public_modifier?(node.parent) ||
    /// preceding_non_public_modifier?(node)`.
    fn non_public(&self) -> bool {
        if let Some(modifier) = &self.parent_modifier {
            // `private_class_method` is no visibility scope: only `non_public_modifier?` sees it.
            return modifier.as_slice() != b"public";
        }
        let name = self.def.name();
        let name = name.as_slice();
        // `node_visibility_from_visibility_inline_on_method_name`
        let inline = self.after.iter().rev().find_map(|sibling| {
            let call = scope_call(sibling)?;
            let arguments: Vec<Node<'_>> = call.arguments()?.arguments().iter().collect();
            let [only] = arguments.as_slice() else { return None };
            if call.block().is_some() || only.as_symbol_node()?.unescaped() != name {
                return None;
            }
            Some(call.name().as_slice().to_vec())
        });
        // `node_visibility_from_visibility_block`
        let visibility = inline.or_else(|| {
            self.before.iter().rev().find_map(|sibling| {
                let call = scope_call(sibling)?;
                if call.arguments().is_some() || call.block().is_some() {
                    return None;
                }
                Some(call.name().as_slice().to_vec())
            })
        });
        visibility.is_some_and(|visibility| visibility != b"public")
    }
}

/// `(send nil? {:private :protected :public} ...)`.
fn scope_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    (call.receiver().is_none() && VISIBILITY_SCOPES.contains(&call.name().as_slice()))
        .then_some(call)
}

/// `class_def_nodes`: the `def`s that are children of the class body (a lone
/// body node is one itself, or the node whose children are searched).
fn class_def_nodes<'pr>(class: &ClassNode<'pr>) -> Vec<ClassDef<'pr>> {
    let Some(body) = class.body() else { return Vec::new() };
    let header_end = class.superclass().map_or(class.constant_path().span().end, |s| s.span().end);
    let plain = |def: DefNode<'pr>| ClassDef {
        def,
        before: Vec::new(),
        after: Vec::new(),
        prev_end: header_end,
        parent_modifier: None,
    };
    if let Some(statements) = body.as_statements_node() {
        let list: Vec<Node<'pr>> = statements.body().iter().collect();
        if let [single] = list.as_slice() {
            return single_child_defs(single, header_end);
        }
        let mut out = Vec::new();
        for (index, stmt) in list.iter().enumerate() {
            let Some(def) = stmt.as_def_node().filter(|def| def.receiver().is_none()) else {
                continue;
            };
            out.push(ClassDef {
                def,
                before: list[..index].to_vec(),
                after: list[index + 1..].to_vec(),
                prev_end: if index == 0 { header_end } else { list[index - 1].span().end },
                parent_modifier: None,
            });
        }
        return out;
    }
    // `rescue`/`ensure` bodies: only a lone `def` in the protected body is a child.
    if let Some(statements) = body.as_begin_node().and_then(|begin| begin.statements()) {
        let list: Vec<Node<'pr>> = statements.body().iter().collect();
        if let [single] = list.as_slice() {
            return single
                .as_def_node()
                .filter(|def| def.receiver().is_none())
                .map(plain)
                .into_iter()
                .collect();
        }
    }
    Vec::new()
}

/// `class_def.def_type? ? [class_def] : class_def.each_child_node(:def)` for a
/// class body of exactly one statement.
fn single_child_defs<'pr>(node: &Node<'pr>, header_end: u32) -> Vec<ClassDef<'pr>> {
    let plain = |def: DefNode<'pr>, parent_modifier: Option<Vec<u8>>| ClassDef {
        def,
        before: Vec::new(),
        after: Vec::new(),
        prev_end: header_end,
        parent_modifier,
    };
    if let Some(def) = node.as_def_node() {
        return if def.receiver().is_none() { vec![plain(def, None)] } else { Vec::new() };
    }
    let Some(call) = node.as_call_node() else { return Vec::new() };
    let free_def =
        |candidate: &Node<'pr>| candidate.as_def_node().filter(|d| d.receiver().is_none());
    if let Some(block) = call.block().and_then(|block| block.as_block_node()) {
        // A `block` node's children are the send, the arguments and the body.
        if let Some(body) = block.body().and_then(|body| body.as_statements_node()) {
            let list: Vec<Node<'pr>> = body.body().iter().collect();
            if let [single] = list.as_slice() {
                return free_def(single).map(|def| plain(def, None)).into_iter().collect();
            }
        }
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some(receiver) = call.receiver() {
        out.extend(free_def(&receiver).map(|def| plain(def, None)));
    }
    if let Some(arguments) = call.arguments() {
        let list: Vec<Node<'pr>> = arguments.arguments().iter().collect();
        for argument in &list {
            let Some(def) = free_def(argument) else { continue };
            // `(send nil? {:private :protected :private_class_method} (def ...))`
            // and `(send nil? scopes def)`: the call has this `def` as its only argument.
            let name = call.name();
            let modifier = (call.receiver().is_none()
                && list.len() == 1
                && (VISIBILITY_SCOPES.contains(&name.as_slice())
                    || name.as_slice() == b"private_class_method"))
                .then(|| name.as_slice().to_vec());
            out.push(plain(def, modifier));
        }
    }
    out
}

/// `range_with_comments_and_lines`: the `def` plus the comments the `parser`
/// gem's associator hands it (the ones between the previously visited node and
/// the `def`, other than a trailing comment on that node's last line), extended
/// to whole lines including the final newline.
fn range_with_comments_and_lines(ctx: &Context<'_>, node: Span, prev_end: u32) -> Span {
    let prev_line = ctx.line_col(prev_end).line;
    let start = ctx
        .comments()
        .iter()
        .filter(|c| c.span.start >= prev_end && c.span.end <= node.start && c.line > prev_line)
        .map(|c| c.span.start)
        .min()
        .map_or(node.start, |comment_start| comment_start.min(node.start));
    ctx.whole_lines(Span::new(start, node.end))
}
