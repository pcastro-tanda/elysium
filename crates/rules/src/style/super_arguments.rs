//! `Style/SuperArguments`, ported from RuboCop's
//! `lib/rubocop/cop/style/super_arguments.rb`.
//!
//! Unlike whitequark (where a directly-attached literal block wraps the
//! `:super` node as a separate `:block` ancestor, and `on_super`'s node
//! excludes that wrapping block from its own span), Prism's `SuperNode`
//! embeds a directly-attached literal block as its own `block` field
//! (`BlockNode`) -- as does a trailing block-pass (`&blk`), there as a
//! `BlockArgumentNode` instead of as a regular argument -- and the node's
//! span extends through either. `block_sends_to_super?`'s default-parent
//! check (`super_node.parent` being a block that wraps the call directly)
//! becomes `sup.block()` being a `BlockNode`; the offense/fix range is
//! computed excluding that embedded block, mirroring
//! `ext::call_span_excluding_block` for `CallNode`.
//!
//! `find_def_node`'s ancestor walk (breaking at any block/lambda ancestor
//! whose own call doesn't reach back to the `super` node, continuing past
//! one that does) has no `ctx.ancestors()` equivalent with real node
//! references, so it is reimplemented as a single top-down search from the
//! file root that tracks real ancestor nodes. Since Prism already embeds a
//! directly-attached block inside `SuperNode` itself, any `BlockNode`/
//! `LambdaNode` actually encountered while descending towards a `super`
//! node means the call is genuinely nested inside that block's *body* (the
//! "DSL"/`define_singleton_method` scenario) -- structurally the case
//! upstream's `block_sends_to_super?` always resolves to `false` for, so a
//! block/lambda ancestor always stops the search here.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ArgumentsNode, DefNode, SuperNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Call `super` without arguments and parentheses when the signature is identical.";
const MSG_INLINE_BLOCK: &str = "Call `super` without arguments and parentheses when all \
positional and keyword arguments are forwarded.";

/// Call `super` without arguments and parentheses when the signature is identical.
#[derive(Debug, Clone)]
pub struct SuperArguments;

impl Rule for SuperArguments {
    const META: RuleMeta = RuleMeta {
        name: "Style/SuperArguments",
        department: Department::Style,
        summary: "Call `super` without arguments and parentheses when the signature is identical.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::SuperNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(sup) = node.as_super_node() else { return };
        let root = ctx.parsed().root();
        let Some(def_node) = find_def_node(&root, node.span()) else { return };

        let def_args = def_arg_list(&def_node);
        let super_args = preprocess_super_args(sup.arguments(), sup.block());
        if !arguments_identical(&def_node, &sup, &def_args, &super_args) {
            return;
        }

        let message = if def_args.len() == super_args.len() { MSG } else { MSG_INLINE_BLOCK };
        let report_span = super_span_excluding_block(&sup);
        ctx.report_with_fix(
            &Self::META,
            report_span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(report_span, b"super".to_vec())],
            },
        );
    }
}

/// `find_def_node`: the nearest enclosing `def`/`defs`, or `None` if a
/// block/lambda boundary is crossed first.
fn find_def_node<'pr>(root: &Node<'pr>, target: Span) -> Option<DefNode<'pr>> {
    let mut stack: Vec<Node<'pr>> = Vec::new();
    find_def_node_rec(root, target, &mut stack)
}

fn find_def_node_rec<'pr>(
    node: &Node<'pr>,
    target: Span,
    stack: &mut Vec<Node<'pr>>,
) -> Option<DefNode<'pr>> {
    if node.kind() == NodeKind::SuperNode && node.span() == target {
        return resolve_enclosing_def(stack);
    }
    stack.push(*node);
    let mut found = None;
    ruby_ast::for_each_child(node, |child| {
        if found.is_none() {
            found = find_def_node_rec(child, target, stack);
        }
    });
    stack.pop();
    found
}

fn resolve_enclosing_def<'pr>(stack: &[Node<'pr>]) -> Option<DefNode<'pr>> {
    for ancestor in stack.iter().rev() {
        match ancestor.kind() {
            NodeKind::DefNode => return ancestor.as_def_node(),
            NodeKind::BlockNode | NodeKind::LambdaNode => return None,
            _ => {}
        }
    }
    None
}

/// `def_node.arguments.argument_list`: every parameter, in declaration
/// order (Ruby's fixed required/optional/rest/post/keyword/kwrest/block
/// order matches `ParametersNode`'s own field order).
fn def_arg_list<'pr>(def_node: &DefNode<'pr>) -> Vec<Node<'pr>> {
    let Some(params) = def_node.parameters() else { return Vec::new() };
    let mut list: Vec<Node<'pr>> = Vec::new();
    list.extend(params.requireds().iter());
    list.extend(params.optionals().iter());
    if let Some(rest) = params.rest() {
        list.push(rest);
    }
    list.extend(params.posts().iter());
    list.extend(params.keywords().iter());
    if let Some(kwrest) = params.keyword_rest() {
        list.push(kwrest);
    }
    if let Some(block) = params.block() {
        list.push(block.as_node());
    }
    list
}

/// `preprocess_super_args`: a bare (braceless) trailing keyword hash
/// (`KeywordHashNode`) is flattened into its individual pairs/splats; an
/// explicit `{ ... }` hash literal (`HashNode`) is left intact. Prism puts
/// a trailing block-pass (`&blk`) into `SuperNode#block` rather than
/// `#arguments` (unlike a literal `{ }`/`do...end` block, which also lands
/// there as a `BlockNode`), so it is appended here to match whitequark's
/// argument list, which includes `block_pass` but not a wrapping `:block`.
fn preprocess_super_args<'pr>(
    args: Option<ArgumentsNode<'pr>>,
    block: Option<Node<'pr>>,
) -> Vec<Node<'pr>> {
    let mut list = Vec::new();
    if let Some(args) = args {
        for arg in &args.arguments() {
            if let Some(kw) = arg.as_keyword_hash_node() {
                list.extend(kw.elements().iter());
            } else {
                list.push(arg);
            }
        }
    }
    if let Some(block) = block {
        if block.kind() == NodeKind::BlockArgumentNode {
            list.push(block);
        }
    }
    list
}

fn arguments_identical<'pr>(
    def_node: &DefNode<'pr>,
    sup: &SuperNode<'pr>,
    def_args: &[Node<'pr>],
    super_args: &[Node<'pr>],
) -> bool {
    if argument_list_size_differs(def_args, super_args, sup) {
        return false;
    }
    def_args.iter().zip(super_args.iter()).all(|(def_arg, super_arg)| {
        positional_arg_same(def_arg, super_arg)
            || positional_rest_arg_same(def_arg, super_arg)
            || keyword_arg_same(def_arg, super_arg)
            || keyword_rest_arg_same(def_arg, super_arg)
            || block_arg_same(def_node, sup, def_arg, super_arg)
            || forward_arg_same(def_arg, super_arg)
    })
}

fn argument_list_size_differs(
    def_args: &[Node<'_>],
    super_args: &[Node<'_>],
    sup: &SuperNode<'_>,
) -> bool {
    let mut def_args_size = def_args.len();
    if def_args.iter().any(|a| a.kind() == NodeKind::BlockParameterNode)
        && block_sends_to_super(sup)
    {
        def_args_size -= 1;
    }
    def_args_size != super_args.len()
}

/// `block_sends_to_super?(super_node)` (default parent, i.e. a block
/// attached directly to the `super(...)` call itself): Prism embeds such a
/// block as `SuperNode#block` rather than as a separate wrapping ancestor.
fn block_sends_to_super(sup: &SuperNode<'_>) -> bool {
    sup.block().is_some_and(|b| b.kind() == NodeKind::BlockNode)
}

fn positional_arg_same(def_arg: &Node<'_>, super_arg: &Node<'_>) -> bool {
    let name = if let Some(req) = def_arg.as_required_parameter_node() {
        req.name()
    } else if let Some(opt) = def_arg.as_optional_parameter_node() {
        opt.name()
    } else {
        return false;
    };
    let Some(lvar) = super_arg.as_local_variable_read_node() else { return false };
    name.as_slice() == lvar.name().as_slice()
}

fn positional_rest_arg_same(def_arg: &Node<'_>, super_arg: &Node<'_>) -> bool {
    let Some(rest) = def_arg.as_rest_parameter_node() else { return false };
    if rest.name().is_none() {
        // Anonymous forwarding: `def foo(*); super(*); end`.
        return super_arg.as_splat_node().is_some_and(|s| s.expression().is_none());
    }
    let Some(splat) = super_arg.as_splat_node() else { return false };
    let Some(lvar) = splat.expression().and_then(|e| e.as_local_variable_read_node()) else {
        return false;
    };
    rest.name().is_some_and(|n| n.as_slice() == lvar.name().as_slice())
}

fn keyword_arg_same(def_arg: &Node<'_>, super_arg: &Node<'_>) -> bool {
    let name = if let Some(req) = def_arg.as_required_keyword_parameter_node() {
        req.name()
    } else if let Some(opt) = def_arg.as_optional_keyword_parameter_node() {
        opt.name()
    } else {
        return false;
    };
    let Some(assoc) = super_arg.as_assoc_node() else { return false };
    let Some(sym) = assoc.key().as_symbol_node() else { return false };
    // `a:` hash-value shorthand (3.1+) wraps the implied `a` in an
    // `ImplicitNode`, unlike the explicit `a: a` form.
    let value = match assoc.value().as_implicit_node() {
        Some(implicit) => implicit.value(),
        None => assoc.value(),
    };
    let Some(lvar) = value.as_local_variable_read_node() else { return false };
    sym.unescaped() == lvar.name().as_slice() && name.as_slice() == sym.unescaped()
}

fn keyword_rest_arg_same(def_arg: &Node<'_>, super_arg: &Node<'_>) -> bool {
    let Some(kwrest) = def_arg.as_keyword_rest_parameter_node() else { return false };
    if kwrest.name().is_none() {
        // Anonymous forwarding: `def foo(**); super(**); end`.
        return super_arg.as_assoc_splat_node().is_some_and(|s| s.value().is_none());
    }
    let Some(splat) = super_arg.as_assoc_splat_node() else { return false };
    let Some(lvar) = splat.value().and_then(|v| v.as_local_variable_read_node()) else {
        return false;
    };
    kwrest.name().is_some_and(|n| n.as_slice() == lvar.name().as_slice())
}

fn block_arg_same<'pr>(
    def_node: &DefNode<'pr>,
    sup: &SuperNode<'pr>,
    def_arg: &Node<'pr>,
    super_arg: &Node<'pr>,
) -> bool {
    let Some(block_param) = def_arg.as_block_parameter_node() else { return false };
    if block_sends_to_super(sup) {
        return true;
    }
    let Some(block_pass) = super_arg.as_block_argument_node() else { return false };
    match block_pass.expression() {
        None => block_param.name().is_none(),
        Some(expr) => {
            let Some(lvar) = expr.as_local_variable_read_node() else { return false };
            let Some(param_name) = block_param.name() else { return false };
            param_name.as_slice() == lvar.name().as_slice()
                && !block_reassigned(def_node, lvar.name().as_slice())
        }
    }
}

/// Reassigning the block argument (`ASSIGN_TYPES = [:or_asgn, :lvasgn]`)
/// will still pass along the original block to `super`.
fn block_reassigned(def_node: &DefNode<'_>, name: &[u8]) -> bool {
    let Some(body) = def_node.body() else { return false };
    if node_reassigns(&body, name) {
        return true;
    }
    let mut found = false;
    ruby_ast::each_descendant(&body, &mut |node| {
        if !found && node_reassigns(node, name) {
            found = true;
        }
    });
    found
}

fn node_reassigns(node: &Node<'_>, name: &[u8]) -> bool {
    if let Some(write) = node.as_local_variable_write_node() {
        return write.name().as_slice() == name;
    }
    if let Some(write) = node.as_local_variable_or_write_node() {
        return write.name().as_slice() == name;
    }
    false
}

fn forward_arg_same(def_arg: &Node<'_>, super_arg: &Node<'_>) -> bool {
    def_arg.kind() == NodeKind::ForwardingParameterNode
        && super_arg.kind() == NodeKind::ForwardingArgumentsNode
}

fn super_end_excluding_block(sup: &SuperNode<'_>) -> u32 {
    if let Some(rparen) = sup.rparen_loc() {
        return rparen.span().end;
    }
    if let Some(args) = sup.arguments() {
        if let Some(last) = args.arguments().iter().last() {
            return last.span().end;
        }
    }
    sup.keyword_loc().span().end
}

/// `sup`'s own span, excluding any directly-attached block -- the offense
/// and fix range both use this, mirroring `ext::call_span_excluding_block`.
fn super_span_excluding_block(sup: &SuperNode<'_>) -> Span {
    Span::new(sup.keyword_loc().span().start, super_end_excluding_block(sup))
}
