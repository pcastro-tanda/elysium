//! `Style/Attr`, ported from RuboCop's
//! `lib/rubocop/cop/style/attr.rb`.
//!
//! Upstream's `allowed_context?` walks the node's ancestors for the closest
//! `class` or `block` node (whitequark's `each_ancestor(:class, :block)`,
//! which -- notably -- does *not* match `module` nodes). In Prism there is
//! no ready-made ancestor accessor with that information (message name,
//! block target), so this file re-derives the same ancestor chain with a
//! single top-down search from the tree root, mirroring the whitequark
//! semantics field for field.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{each_descendant, for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Do not use `attr`. Use `%s` instead.";

/// Checks for uses of Module#attr.
#[derive(Debug, Clone)]
pub struct Attr;

impl Rule for Attr {
    const META: RuleMeta = RuleMeta {
        name: "Style/Attr",
        department: Department::Style,
        summary: "Checks for uses of Module#attr.",
        explanation: "Checks for uses of `Module#attr`. The `attr` method has \
            confusing behavior: with a single argument it creates a reader \
            (like `attr_reader`), but with a second boolean argument it \
            creates an accessor (deprecated in Ruby 1.9). Use `attr_reader` \
            or `attr_accessor` to make intent explicit.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
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
        if call.receiver().is_some() {
            return;
        }
        if call.name().as_slice() != b"attr" {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
        if args.is_empty() {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };

        let root = ctx.parsed().root();
        let found = match find_context(&root, node.span(), None) {
            Search::Found(ctx) => ctx,
            Search::NotFound => None,
        };
        if allowed_context(found.as_ref()) {
            return;
        }

        let last_arg = args.last().expect("checked non-empty above");
        let replacement = replacement_method(last_arg);
        let message = MSG.replacen("%s", replacement, 1);

        let mut edits = vec![Edit::replace(message_loc.span(), replacement.as_bytes().to_vec())];

        let first_arg = &args[0];
        let setter = args.get(1);
        if setter.is_some_and(is_bool_literal) {
            let remove_span = Span::new(first_arg.span().end, node.span().end);
            edits.push(Edit::delete(remove_span));
        }

        ctx.report_with_fix(
            &Self::META,
            message_loc.span(),
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

fn is_bool_literal(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::TrueNode | NodeKind::FalseNode)
}

/// `replacement_method`: keyed off the *last* argument of the whole call,
/// not the (up to two) destructured in `autocorrect` below.
fn replacement_method(last_arg: &Node<'_>) -> &'static str {
    match last_arg.kind() {
        NodeKind::TrueNode => "attr_accessor",
        _ => "attr_reader",
    }
}

/// The ancestor context `allowed_context?` inspects: the innermost
/// enclosing `class` or `block` node (whitequark's `:class`/`:block`
/// types), plus whether that block is a `class_eval`/`module_eval` call.
#[derive(Clone, Copy)]
struct AncestorCtx<'pr> {
    is_class: bool,
    is_class_eval: bool,
    node: Node<'pr>,
}

/// `allowed_context?`: no enclosing class/block at all means the offense
/// fires (whitequark's `return false unless (class_node = ...)`).
fn allowed_context(ctx: Option<&AncestorCtx<'_>>) -> bool {
    match ctx {
        None => false,
        Some(a) => (!a.is_class && !a.is_class_eval) || has_attr_def(&a.node),
    }
}

/// `define_attr_method?`: any descendant `def` (including singleton defs)
/// named `attr`, anywhere under the ancestor node.
fn has_attr_def(node: &Node<'_>) -> bool {
    let mut found = false;
    each_descendant(node, &mut |child| {
        if found {
            return;
        }
        if let Some(def) = child.as_def_node() {
            if def.name().as_slice() == b"attr" {
                found = true;
            }
        }
    });
    found
}

/// Result of [`find_context`]: whether `target` was reached in the
/// subtree, and if so, its ancestor context (an `Option` in its own
/// right, since "no enclosing class/block" is itself a valid outcome).
enum Search<'pr> {
    NotFound,
    Found(Option<AncestorCtx<'pr>>),
}

/// Top-down search for `target`'s (the `attr` call's) closest enclosing
/// `class` or `block` ancestor, replicating
/// `node.each_ancestor(:class, :block).first`.
fn find_context<'pr>(node: &Node<'pr>, target: Span, ctx: Option<AncestorCtx<'pr>>) -> Search<'pr> {
    if node.kind() == NodeKind::CallNode && node.span() == target {
        return Search::Found(ctx);
    }

    let next_ctx = if node.kind() == NodeKind::ClassNode {
        Some(AncestorCtx { is_class: true, is_class_eval: false, node: *node })
    } else {
        ctx
    };

    if let Some(call) = node.as_call_node() {
        if let Some(receiver) = call.receiver() {
            if let found @ Search::Found(_) = find_context(&receiver, target, next_ctx) {
                return found;
            }
        }
        if let Some(arguments) = call.arguments() {
            for arg in &arguments.arguments() {
                if let found @ Search::Found(_) = find_context(&arg, target, next_ctx) {
                    return found;
                }
            }
        }
        if let Some(block) = call.block() {
            let block_ctx = if block.kind() == NodeKind::BlockNode {
                let is_eval = !call.is_safe_navigation()
                    && matches!(call.name().as_slice(), b"class_eval" | b"module_eval");
                Some(AncestorCtx { is_class: false, is_class_eval: is_eval, node: block })
            } else {
                next_ctx
            };
            if let found @ Search::Found(_) = find_context(&block, target, block_ctx) {
                return found;
            }
        }
        return Search::NotFound;
    }

    let mut result = Search::NotFound;
    for_each_child(node, |child| {
        if matches!(result, Search::Found(_)) {
            return;
        }
        if let found @ Search::Found(_) = find_context(child, target, next_ctx) {
            result = found;
        }
    });
    result
}
