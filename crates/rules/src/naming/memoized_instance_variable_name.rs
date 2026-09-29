//! `Naming/MemoizedInstanceVariableName`, ported from RuboCop's
//! `lib/rubocop/cop/naming/memoized_instance_variable_name.rb`.
//!
//! # Node mapping
//!
//! Upstream matches two shapes by walking each `ivasgn`/`defined?` node's
//! ancestors for the nearest `def`/`defs`/dynamic-`define_method` block
//! (`find_definition`). Since [`Context::ancestors`] only exposes kind and
//! span, this port instead performs its own top-down walk
//! ([`walk_node`]), pushing a "method frame" (the enclosing body plus the
//! method name) whenever it descends into a `def`/`defs` ([`DefNode`]) or a
//! `define_method`/`define_singleton_method` call's block. The frame on top
//! of the stack when an `@ivar ||= ...` or `defined?(@ivar)` node is
//! reached is exactly upstream's nearest matching ancestor: non-defining
//! blocks (e.g. `.each do ... end`) are walked through without pushing a
//! frame, so the search transparently continues to their enclosing method,
//! matching `each_ancestor(:any_def, :block).each { |a| method_definition?(a) }`
//! skipping non-matches.
//!
//! Prism always wraps a method body in a [`NodeKind::StatementsNode`], even
//! for a single statement, where whitequark elides `begin` in that case.
//! This collapses upstream's two placement checks
//! (`body == node || body.children.last == node`) into one: "is `node` the
//! last statement of the body's `StatementsNode`". Likewise, upstream's
//! `defined_memoized?` node pattern requires the ancestor's body to be a
//! `begin` node (i.e. at least two statements); a single-statement body
//! (`return @x if defined?(@x)` alone) is never `begin`-shaped, so it never
//! matches -- reproduced here by requiring at least two statements.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const DYNAMIC_DEFINE_METHODS: &[&[u8]] = &[b"define_method", b"define_singleton_method"];
const INITIALIZE_METHODS: &[&[u8]] =
    &[b"initialize", b"initialize_clone", b"initialize_copy", b"initialize_dup"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Disallowed,
    Required,
    Optional,
}

/// One enclosing `def`/`defs`/dynamic-`define_method` block: its body (to
/// check placement) and its method name (to check the ivar naming).
struct Frame<'pr> {
    body: Option<Node<'pr>>,
    name: Vec<u8>,
}

/// Memoized method name should match memo instance variable name.
#[derive(Debug, Clone)]
pub struct MemoizedInstanceVariableName {
    style: Style,
}

impl Rule for MemoizedInstanceVariableName {
    const META: RuleMeta = RuleMeta {
        name: "Naming/MemoizedInstanceVariableName",
        department: Department::Naming,
        summary: "Memoized method name should match memo instance variable name.",
        explanation: "Checks for memoized methods whose instance variable name \
                      does not match the method name. Applies to both regular methods \
                      (defined with `def`) and dynamic methods (defined with \
                      `define_method` or `define_singleton_method`).\n\n\
                      This cop can be configured with the `EnforcedStyleForLeadingUnderscores` \
                      directive. It can be configured to allow for memoized instance variables \
                      prefixed with an underscore. Prefixing ivars with an underscore is a \
                      convention that is used to implicitly indicate that an ivar should not \
                      be set or referenced outside of the memoization method.\n\n\
                      This cop relies on the pattern `@instance_var ||= ...`, but this is \
                      sometimes used for other purposes than memoization so this cop is \
                      considered unsafe. Also, its autocorrection is unsafe because it may \
                      conflict with instance variable names already in use.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[linter::ConfigOption {
            name: "EnforcedStyleForLeadingUnderscores",
            default: linter::ConfigDefault::Str("disallowed"),
            allowed: &["disallowed", "required", "optional"],
            doc: "Whether memoized instance variables are required, allowed, or forbidden \
                  to have a leading underscore.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyleForLeadingUnderscores")? {
            "required" => Style::Required,
            "optional" => Style::Optional,
            _ => Style::Disallowed,
        };
        Ok(Self { style })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let mut stack: Vec<Frame<'_>> = Vec::new();
        walk_node(&root, &mut stack, self.style, ctx);
    }
}

/// Top-down walk maintaining the enclosing-method-frame stack described in
/// the module doc comment.
fn walk_node<'pr>(
    node: &Node<'pr>,
    stack: &mut Vec<Frame<'pr>>,
    style: Style,
    ctx: &mut Context<'pr>,
) {
    if let Some(def) = node.as_def_node() {
        stack.push(Frame { body: def.body(), name: def.name().as_slice().to_vec() });
        if let Some(body) = def.body() {
            walk_node(&body, stack, style, ctx);
        }
        stack.pop();
        return;
    }

    if let Some(call) = node.as_call_node() {
        if let Some(receiver) = call.receiver() {
            walk_node(&receiver, stack, style, ctx);
        }
        if let Some(args) = call.arguments() {
            for arg in &args.arguments() {
                walk_node(&arg, stack, style, ctx);
            }
        }
        if let Some(block_child) = call.block() {
            if let Some((name, block)) = dynamic_define_block(&call, &block_child) {
                stack.push(Frame { body: block.body(), name });
                if let Some(body) = block.body() {
                    walk_node(&body, stack, style, ctx);
                }
                stack.pop();
            } else {
                walk_node(&block_child, stack, style, ctx);
            }
        }
        return;
    }

    match node.kind() {
        NodeKind::InstanceVariableOrWriteNode => check_or_write(node, stack, style, ctx),
        NodeKind::DefinedNode => check_defined(node, stack, style, ctx),
        _ => {}
    }
    for_each_child(node, |child| walk_node(child, stack, style, ctx));
}

/// Matches upstream's `(block (send _ %DYNAMIC_DEFINE_METHODS ({sym str} $_)) ...)`:
/// `call` is `define_method`/`define_singleton_method` with a literal
/// symbol/string first argument, and `block_child` is its block (as
/// opposed to a `&blk` [`ruby_ast::ext`] block-argument pass-through).
fn dynamic_define_block<'pr>(
    call: &ruby_ast::node::CallNode<'pr>,
    block_child: &Node<'pr>,
) -> Option<(Vec<u8>, ruby_ast::node::BlockNode<'pr>)> {
    if !DYNAMIC_DEFINE_METHODS.contains(&call.name().as_slice()) {
        return None;
    }
    let block = block_child.as_block_node()?;
    let first = call.arguments()?.arguments().iter().next()?;
    let name = match first.kind() {
        NodeKind::SymbolNode => first.as_symbol_node()?.unescaped().to_vec(),
        NodeKind::StringNode => first.as_string_node()?.unescaped().to_vec(),
        _ => return None,
    };
    Some((name, block))
}

/// True when `node` (identified by its span) is upstream's
/// `body == node || body.children.last == node`.
///
/// For a multi-statement body, Prism's `StatementsNode` list mirrors
/// whitequark's `begin` node's children directly: `body.children.last` is
/// just that list's last entry, compared without recursing into it.
///
/// For a *single*-statement body, whitequark elides the `begin` wrapper
/// entirely -- `body` upstream *is* that one statement, so both checks
/// apply to it: `body == node` (the statement itself) and
/// `body.children.last == node`, which requires reproducing whitequark's
/// per-node-kind child list. Only `if`/`unless` are implemented (the only
/// kinds a real memoization guard clause appears in): whitequark unifies
/// both into one `:if` node with three positional children
/// `(cond, if_true, if_false)`, swapping `if_true`/`if_false` for
/// `unless`; the last child is `if_false`, i.e. an `if`'s `else`/`elsif`
/// branch, or an `unless`'s own body. A branch holding more than one
/// statement is itself whitequark's `begin` node, compared as a whole (so
/// it can only ever match a `node` spanning that whole branch, never one
/// of its individual statements) -- reachable here by returning that
/// branch's sole statement only when there is exactly one, otherwise
/// `None` (the branch, whatever it is, never has `node`'s narrower span).
/// Any other body kind is left unhandled (never matches, i.e. no
/// offense) rather than guessing at its whitequark child list.
fn is_last_statement(body: Option<Node<'_>>, node_span: Span) -> bool {
    let Some(stmts) = body.and_then(|b| b.as_statements_node()) else { return false };
    let list = stmts.body();
    let mut iter = list.iter();
    let Some(first) = iter.next() else { return false };
    if iter.next().is_some() {
        return list.iter().last().is_some_and(|last| last.span() == node_span);
    }
    if first.span() == node_span {
        return true;
    }
    whitequark_last_child(&first).is_some_and(|child| child.span() == node_span)
}

/// See [`is_last_statement`]: whitequark's `children.last` for a
/// single-statement `if`/`unless` body.
fn whitequark_last_child<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    if let Some(if_node) = node.as_if_node() {
        let subsequent = if_node.subsequent()?;
        return match subsequent.as_else_node() {
            Some(else_node) => single_statement(else_node.statements()),
            // `elsif`: the nested `if` node sits directly in the
            // `if_false` slot in whitequark's tree.
            None => Some(subsequent),
        };
    }
    if let Some(unless_node) = node.as_unless_node() {
        return single_statement(unless_node.statements());
    }
    None
}

fn single_statement(stmts: Option<ruby_ast::node::StatementsNode<'_>>) -> Option<Node<'_>> {
    let mut iter = stmts?.body().iter();
    let first = iter.next()?;
    iter.next().is_none().then_some(first)
}

fn check_or_write<'pr>(
    node: &Node<'pr>,
    stack: &[Frame<'pr>],
    style: Style,
    ctx: &mut Context<'pr>,
) {
    let or_write = node.as_instance_variable_or_write_node().expect("kind matched");
    let Some(frame) = stack.last() else { return };
    if !nameable_method(&frame.name) {
        return;
    }
    if !is_last_statement(frame.body, node.span()) {
        return;
    }

    let var = or_write.name().as_slice();
    if names_match(&frame.name, var, style) {
        return;
    }

    report(
        ctx,
        &MemoizedInstanceVariableName::META,
        or_write.name_loc().span(),
        var,
        &frame.name,
        style,
    );
}

fn check_defined<'pr>(
    node: &Node<'pr>,
    stack: &[Frame<'pr>],
    style: Style,
    ctx: &mut Context<'pr>,
) {
    let defined = node.as_defined_node().expect("kind matched");
    let Some(arg) = defined.value().as_instance_variable_read_node() else { return };

    let Some(frame) = stack.last() else { return };
    if !nameable_method(&frame.name) {
        return;
    }

    let Some((defined_ivar, return_ivar, ivar_assign)) =
        defined_memoized(frame.body, arg.name().as_slice())
    else {
        return;
    };

    let var = ivar_assign.name().as_slice();
    if names_match(&frame.name, var, style) {
        return;
    }

    report(ctx, &MemoizedInstanceVariableName::META, defined_ivar.span(), var, &frame.name, style);
    report(ctx, &MemoizedInstanceVariableName::META, return_ivar.span(), var, &frame.name, style);
    report(
        ctx,
        &MemoizedInstanceVariableName::META,
        ivar_assign.name_loc().span(),
        var,
        &frame.name,
        style,
    );
}

/// Upstream's `defined_memoized?` node pattern:
/// ```text
/// (begin
///   (if (defined $(ivar %1)) (return $(ivar %1)) nil?)
///   ...
///   $(ivasgn %1 _))
/// ```
/// Returns the `@ivar` inside `defined?(...)`, the `@ivar` after `return`,
/// and the trailing plain assignment, in that order.
fn defined_memoized<'pr>(
    body: Option<Node<'pr>>,
    ivar_name: &[u8],
) -> Option<(Node<'pr>, Node<'pr>, ruby_ast::node::InstanceVariableWriteNode<'pr>)> {
    let stmts = body?.as_statements_node()?;
    let statements = stmts.body();
    if statements.iter().count() < 2 {
        return None;
    }

    let first = statements.iter().next()?;
    let if_node = first.as_if_node()?;
    if if_node.subsequent().is_some() {
        return None;
    }
    let defined = if_node.predicate().as_defined_node()?;
    let defined_ivar = defined.value();
    if defined_ivar.as_instance_variable_read_node()?.name().as_slice() != ivar_name {
        return None;
    }

    let then_stmts = if_node.statements()?;
    let mut then_iter = then_stmts.body().iter();
    let return_stmt = then_iter.next()?;
    if then_iter.next().is_some() {
        return None;
    }
    let return_node = return_stmt.as_return_node()?;
    let mut return_args = return_node.arguments()?.arguments().iter();
    let return_ivar = return_args.next()?;
    if return_args.next().is_some() {
        return None;
    }
    if return_ivar.as_instance_variable_read_node()?.name().as_slice() != ivar_name {
        return None;
    }

    let final_stmt = statements.iter().last()?;
    let ivar_assign = final_stmt.as_instance_variable_write_node()?;
    if ivar_assign.name().as_slice() != ivar_name {
        return None;
    }

    Some((defined_ivar, return_ivar, ivar_assign))
}

/// Operator and other non-word method names (e.g. `[]`, `+`, `<=>`) cannot
/// form a valid instance variable name, so there is no matching ivar to
/// enforce and a suggested correction like `@[]` would be invalid Ruby.
fn nameable_method(method_name: &[u8]) -> bool {
    let mut chars = strip_bang_question_eq(method_name).into_iter();
    let Some(first) = chars.next() else { return false };
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return false;
    }
    chars.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn strip_bang_question_eq(name: &[u8]) -> Vec<u8> {
    name.iter().copied().filter(|&b| b != b'!' && b != b'?' && b != b'=').collect()
}

fn no_underscore(name: &[u8]) -> &[u8] {
    name.strip_prefix(b"_").unwrap_or(name)
}

fn with_underscore(name: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(name.len() + 1);
    v.push(b'_');
    v.extend_from_slice(name);
    v
}

/// `var` is the ivar name including its leading `@`; `method_name` is the
/// raw (unstripped) method name.
fn names_match(method_name: &[u8], var: &[u8], style: Style) -> bool {
    if INITIALIZE_METHODS.contains(&method_name) {
        return true;
    }
    let method = strip_bang_question_eq(method_name);
    let Some(var) = var.strip_prefix(b"@") else { return false };
    match style {
        Style::Disallowed => var == method.as_slice() || var == no_underscore(&method),
        Style::Required => {
            var == with_underscore(&method).as_slice()
                || (method.starts_with(b"_") && var == method.as_slice())
        }
        Style::Optional => {
            var == method.as_slice()
                || var == with_underscore(&method).as_slice()
                || var == no_underscore(&method)
        }
    }
}

fn suggested_var(method_name: &[u8], style: Style) -> Vec<u8> {
    let stripped = strip_bang_question_eq(method_name);
    if style == Style::Required {
        with_underscore(&stripped)
    } else {
        stripped
    }
}

fn report(
    ctx: &mut Context<'_>,
    meta: &RuleMeta,
    span: Span,
    var: &[u8],
    method_name: &[u8],
    style: Style,
) {
    let var_name = var.strip_prefix(b"@").unwrap_or(var);
    let suggested = suggested_var(method_name, style);
    let message = if style == Style::Required && !var_name.starts_with(b"_") {
        format!(
            "Memoized variable `{}` does not start with `_`. Use `@{}` instead.",
            String::from_utf8_lossy(var),
            String::from_utf8_lossy(&suggested),
        )
    } else {
        format!(
            "Memoized variable `{}` does not match method name `{}`. Use `@{}` instead.",
            String::from_utf8_lossy(var),
            String::from_utf8_lossy(method_name),
            String::from_utf8_lossy(&suggested),
        )
    };
    let mut replacement = Vec::with_capacity(suggested.len() + 1);
    replacement.push(b'@');
    replacement.extend_from_slice(&suggested);
    ctx.report_with_fix(
        meta,
        span,
        message,
        Fix { applicability: Applicability::Unsafe, edits: vec![Edit::replace(span, replacement)] },
    );
}
