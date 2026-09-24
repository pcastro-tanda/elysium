//! `Style/SymbolProc`, ported from RuboCop's `lib/rubocop/cop/style/symbol_proc.rb`.
//!
//! RuboCop's `symbol_proc?` node-pattern matches three whitequark shapes --
//! `block`, `numblock`, and `itblock` -- that Prism collapses into one:
//! every `foo { |x| x.bar }` / `foo { _1.bar }` / `foo { it.bar }` is a
//! [`ruby_ast::node::BlockNode`] attached to a [`ruby_ast::node::CallNode`]
//! (or [`ruby_ast::node::SuperNode`]/[`ruby_ast::node::ForwardingSuperNode`])
//! via its `block` field, distinguished only by what `BlockNode::parameters`
//! returns ([`param_shape`]). A lambda literal (`->(x) { x.bar }`) is its
//! own [`ruby_ast::node::LambdaNode`] rather than a call with an attached
//! block, but shares the same parameter/body shape, so [`symbol_proc_body`]
//! is reused for both. This rule therefore subscribes to all four node
//! kinds and does all of its work in a single `enter`, reading the already
//! -built child node (no re-walk, no ancestor lookups needed: the block is
//! a direct field of the node we are visiting).
//!
//! `AllowedMethods`/`AllowedPatterns` gate on the *outer* dispatch method
//! name (`super` for both `super(...)`/`super`, `lambda` for a `->`
//! literal, else the call's own message). `AllowMethodsWithArguments` and
//! the autocorrection both need that dispatch's own argument list and
//! parens, abstracted as [`OuterShape`] so `CallNode`/`SuperNode` share one
//! implementation; `ForwardingSuperNode` (`super` with no parens, ever) is
//! the degenerate case with no parens and no arguments.

use std::borrow::Cow;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::{ArgumentsNode, BlockNode, CallNode, ConstantId};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// What a block/lambda's parameter list declares, once reduced to the one
/// shape RuboCop's `symbol_proc?` pattern allows: a single, non-destructured
/// parameter (named, numbered `_1`, or `it`).
enum Param<'pr> {
    Named(ConstantId<'pr>),
    Numbered,
    It,
}

/// Classifies `parameters` (a `BlockNode`/`LambdaNode`'s `parameters()`),
/// matching RuboCop's three `symbol_proc?` alternatives:
/// `(args (arg _var))` (a lone required parameter -- destructuring, splats,
/// optionals, keywords, a block parameter, or trailing `;` shadow locals
/// all break the pattern and fall through to `None`), `numblock`'s bound
/// `$1` (Prism's `NumberedParametersNode::maximum` -- `_2` used without
/// `_1` bumps this above `1` and is correctly rejected), and `itblock`.
fn param_shape(parameters: Option<Node<'_>>) -> Option<Param<'_>> {
    let params = parameters?;
    match params {
        Node::BlockParametersNode { .. } => {
            let block_params = params.as_block_parameters_node()?;
            if !block_params.locals().is_empty() {
                return None;
            }
            let inner = block_params.parameters()?;
            if !inner.optionals().is_empty()
                || inner.rest().is_some()
                || !inner.posts().is_empty()
                || !inner.keywords().is_empty()
                || inner.keyword_rest().is_some()
                || inner.block().is_some()
                || inner.requireds().len() != 1
            {
                return None;
            }
            let required = inner.requireds().first()?.as_required_parameter_node()?;
            Some(Param::Named(required.name()))
        }
        Node::NumberedParametersNode { .. } => {
            let numbered = params.as_numbered_parameters_node()?;
            (numbered.maximum() == 1).then_some(Param::Numbered)
        }
        Node::ItParametersNode { .. } => Some(Param::It),
        _ => None,
    }
}

/// Whether `receiver` (the body call's receiver) reads exactly the
/// parameter `param` declares, matching RuboCop's bound `_var`/`:_1`/`:it`
/// pattern variables.
fn receiver_matches(receiver: &Node<'_>, param: &Param<'_>) -> bool {
    match param {
        Param::Named(name) => receiver
            .as_local_variable_read_node()
            .is_some_and(|lvar| lvar.name().as_slice() == name.as_slice()),
        Param::Numbered => receiver
            .as_local_variable_read_node()
            .is_some_and(|lvar| lvar.name().as_slice() == b"_1"),
        Param::It => matches!(receiver, Node::ItLocalVariableReadNode { .. }),
    }
}

/// The sole `<param>.method` call making up a block/lambda's body, matching
/// RuboCop's `(send (lvar _var) $_)`: exactly one statement, no arguments,
/// no attached block of its own, and not safe-navigation (whitequark's
/// `send`/`csend` split, which Prism folds into one `CallNode`).
fn body_call(body: Option<Node<'_>>) -> Option<CallNode<'_>> {
    let stmts = body?.as_statements_node()?;
    let stmts_body = stmts.body();
    if stmts_body.len() != 1 {
        return None;
    }
    let call = stmts_body.first()?.as_call_node()?;
    if call.arguments().is_some() || call.block().is_some() || call.is_safe_navigation() {
        return None;
    }
    Some(call)
}

/// The body call's method name (for the message and the fix) once the
/// parameter shape and the body both match; `None` otherwise (including
/// the "no arguments at all" and "empty body" cases, so this alone covers
/// `accepts_block_with_no_arguments`/`accepts_empty_block_body`).
fn symbol_proc_body<'pr>(
    parameters: Option<Node<'pr>>,
    body: Option<Node<'pr>>,
) -> Option<&'pr [u8]> {
    let param = param_shape(parameters)?;
    let call = body_call(body)?;
    let receiver = call.receiver()?;
    receiver_matches(&receiver, &param).then(|| call.name().as_slice())
}

/// RuboCop's `unsafe_hash_usage?`: `reject`/`select` on a hash *literal*
/// receiver would change return type if converted (`Hash#reject(&:sym)`
/// returns an `Array`, not a `Hash`, for a block-free predicate -- wait,
/// actually a `Hash`, but the block arity differs; see the cop's comment).
fn unsafe_hash_usage(call: &CallNode<'_>) -> bool {
    matches!(call.name().as_slice(), b"reject" | b"select")
        && call.receiver().is_some_and(|r| matches!(r, Node::HashNode { .. }))
}

/// RuboCop's `unsafe_array_usage?`: `min`/`max` on an array literal.
fn unsafe_array_usage(call: &CallNode<'_>) -> bool {
    matches!(call.name().as_slice(), b"min" | b"max")
        && call.receiver().is_some_and(|r| matches!(r, Node::ArrayNode { .. }))
}

/// `(const {nil? cbase} :Proc)`: a bare or top-level-qualified `Proc`
/// constant, receiver of `.new`.
fn is_proc_const(node: &Node<'_>) -> bool {
    match node {
        Node::ConstantReadNode { .. } => {
            node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"Proc")
        }
        Node::ConstantPathNode { .. } => node.as_constant_path_node().is_some_and(|path| {
            path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == b"Proc")
        }),
        _ => false,
    }
}

/// RuboCop's `proc_node?`: `Proc.new`/`::Proc.new`.
fn is_proc_new(call: &CallNode<'_>) -> bool {
    call.name().as_slice() == b"new" && call.receiver().is_some_and(|r| is_proc_const(&r))
}

/// A dispatch's own parens/argument-list shape, abstracted over
/// `CallNode`/`SuperNode` (`ForwardingSuperNode` -- bare `super`, no parens,
/// ever -- builds the all-`None` variant directly) so the fix logic that
/// depends on it (`begin_pos_for_replacement`, `call_fix`) is written once.
struct OuterShape<'pr> {
    opening_paren: Option<Span>,
    closing_paren: Option<Span>,
    arguments: Option<ArgumentsNode<'pr>>,
}

impl OuterShape<'_> {
    /// RuboCop's `send_node.arguments?`: any explicit argument at all
    /// (bare keyword arguments arrive as one `KeywordHashNode` argument in
    /// Prism's list, same as any other -- still "has arguments").
    fn has_arguments(&self) -> bool {
        self.arguments.as_ref().is_some_and(|a| !a.arguments().is_empty())
    }
}

/// The effective last argument's span for the fix's insertion point:
/// RuboCop-AST represents bare keyword arguments (`foo(a: 1, b: 2)`) as
/// individual pair arguments, so its `args.last` is the last pair; Prism
/// wraps them all in one `KeywordHashNode` argument, so this unwraps to
/// that hash's own last element to match.
fn last_argument_span(args: &ArgumentsNode<'_>) -> Option<Span> {
    let last = args.arguments().last()?;
    if let Node::KeywordHashNode { .. } = &last {
        return Some(last.as_keyword_hash_node()?.elements().last()?.span());
    }
    Some(last.span())
}

/// RuboCop's `begin_pos_for_replacement`: the position autocorrection
/// starts removing/replacing from -- the dispatch's own opening paren when
/// it has parens with nothing (but whitespace) between them (`foo(   ) { }`
/// -- no `ArgumentsNode` at all, since Prism never builds one with zero
/// elements), else the block/lambda's own opening delimiter.
fn begin_pos_for_replacement(ctx: &Context<'_>, shape: &OuterShape<'_>, block_start: u32) -> u32 {
    if shape.arguments.is_none() {
        if let (Some(open), Some(close)) = (shape.opening_paren, shape.closing_paren) {
            let inner = ctx.text(Span::new(open.end, close.start));
            if inner.iter().all(|&b| matches!(b, b' ' | b'\t' | b'\n' | b'\r')) {
                return open.start;
            }
        }
    }
    block_start
}

/// RuboCop's `block_range_with_space`: `begin_pos_for_replacement` through
/// the block/lambda's own end, extended left over a run of spaces/tabs and
/// then a run of newlines (`RangeHelp#range_with_surrounding_space`'s two
/// sequential passes, `side: :left`).
fn block_range_with_space(ctx: &Context<'_>, begin_pos: u32, end_pos: u32) -> Span {
    let bytes = ctx.source().bytes();
    let mut start = begin_pos as usize;
    while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
        start -= 1;
    }
    while start > 0 && bytes[start - 1] == b'\n' {
        start -= 1;
    }
    Span::new(u32::try_from(start).unwrap_or(u32::MAX), end_pos)
}

/// RuboCop's `autocorrect`/`autocorrect_with_args`/`autocorrect_without_args`
/// for a `CallNode`/`SuperNode`/`ForwardingSuperNode` dispatch (a lambda
/// literal is replaced wholesale instead; see [`lambda_fix`]).
fn call_fix(
    shape: &OuterShape<'_>,
    block_start: u32,
    block_end: u32,
    ctx: &Context<'_>,
    body_method: &[u8],
) -> Fix {
    let begin = begin_pos_for_replacement(ctx, shape, block_start);

    if shape.has_arguments() {
        let args = shape.arguments.as_ref().expect("has_arguments implies Some");
        let arg_span = last_argument_span(args).unwrap_or_else(|| Span::new(begin, begin));
        let bytes = ctx.source().bytes();
        let mut arg_end = arg_span.end;
        if bytes.get(arg_end as usize) == Some(&b',') {
            arg_end += 1;
        }
        let ends_with_comma = arg_end > 0 && bytes.get((arg_end - 1) as usize) == Some(&b',');
        let mut insertion = Vec::with_capacity(body_method.len() + 5);
        if !ends_with_comma {
            insertion.push(b',');
        }
        insertion.extend_from_slice(b" &:");
        insertion.extend_from_slice(body_method);
        let delete_span = block_range_with_space(ctx, begin, block_end);
        Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::insert(arg_end, insertion), Edit::delete(delete_span)],
        }
    } else {
        let delete_span = block_range_with_space(ctx, begin, block_end);
        let mut replacement = Vec::with_capacity(body_method.len() + 4);
        replacement.extend_from_slice(b"(&:");
        replacement.extend_from_slice(body_method);
        replacement.push(b')');
        Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(delete_span, replacement)],
        }
    }
}

/// RuboCop's `autocorrect_without_args` lambda-literal branch: a `->`
/// literal is always replaced wholesale, since `lambda(&:sym)` cannot
/// preserve any parameter-list text (RuboCop's own `else` branch handling
/// a `lambda do end`-shaped selector is unreachable: `lambda_literal?` is
/// true only for `->`).
fn lambda_fix(lambda_span: Span, body_method: &[u8]) -> Fix {
    let mut replacement = Vec::with_capacity(body_method.len() + 9);
    replacement.extend_from_slice(b"lambda(&:");
    replacement.extend_from_slice(body_method);
    replacement.push(b')');
    Fix {
        applicability: Applicability::Unsafe,
        edits: vec![Edit::replace(lambda_span, replacement)],
    }
}

/// Looks for blocks/lambdas that read their sole parameter once, with no
/// other work, and could be a `Symbol#to_proc` instead.
#[derive(Debug, Clone)]
pub struct SymbolProc {
    allow_methods_with_arguments: bool,
    allowed_methods: Vec<String>,
    allowed_patterns: Vec<Regex>,
    allow_comments: bool,
    active_support_extensions_enabled: bool,
}

impl SymbolProc {
    /// RuboCop's `allowed_method_name?`.
    fn allowed_name(&self, name: &[u8]) -> bool {
        if self.allowed_methods.iter().any(|m| m.as_bytes() == name) {
            return true;
        }
        std::str::from_utf8(name)
            .is_ok_and(|text| self.allowed_patterns.iter().any(|re| re.is_match(text)))
    }

    /// RuboCop's `contains_comments?`, restricted to the `any_block_type?`
    /// branch of `find_end_line` (the only one reachable here): any comment
    /// on a line in `[start_line, end_line)` -- the node's own first line
    /// through, but excluding, the block/lambda's closing-delimiter line.
    fn contains_comments(ctx: &Context<'_>, start_offset: u32, end_offset: u32) -> bool {
        let start_line = ctx.line_col(start_offset).line;
        let end_line = ctx.line_col(end_offset).line;
        ctx.comments().iter().any(|c| c.line >= start_line && c.line < end_line)
    }

    fn message(body_method: &[u8], block_method: &[u8]) -> Cow<'static, str> {
        Cow::Owned(format!(
            "Pass `&:{}` as an argument to `{}` instead of a block.",
            String::from_utf8_lossy(body_method),
            String::from_utf8_lossy(block_method),
        ))
    }

    fn check_lambda(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let lambda = node.as_lambda_node().expect("kind matched");
        let Some(body_method) = symbol_proc_body(lambda.parameters(), lambda.body()) else {
            return;
        };
        // `LAMBDA_OR_PROC.include?(dispatch_node.method_name)`: a `->`
        // literal's synthetic dispatch method name is always `:lambda`.
        if self.active_support_extensions_enabled || self.allowed_name(b"lambda") {
            return;
        }
        let close = lambda.closing_loc();
        if self.allow_comments && Self::contains_comments(ctx, node.span().start, close.span().end)
        {
            return;
        }
        let offense = Span::new(lambda.opening_loc().span().start, close.span().end);
        let message = Self::message(body_method, b"lambda");
        let fix = lambda_fix(node.span(), body_method);
        ctx.report_with_fix(&Self::META, offense, message, fix);
    }

    fn check_call(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        let Some(block_node) = call.block() else { return };
        let Node::BlockNode { .. } = &block_node else { return };
        let block = block_node.as_block_node().expect("kind matched");
        let Some(body_method) = symbol_proc_body(block.parameters(), block.body()) else {
            return;
        };

        let method_name = call.name().as_slice();
        if self.active_support_extensions_enabled
            && (is_proc_new(&call) || matches!(method_name, b"lambda" | b"proc"))
        {
            return;
        }
        if unsafe_hash_usage(&call) || unsafe_array_usage(&call) {
            return;
        }
        if self.allowed_name(method_name) {
            return;
        }
        let shape = OuterShape {
            opening_paren: call.opening_loc().map(|l| l.span()),
            closing_paren: call.closing_loc().map(|l| l.span()),
            arguments: call.arguments(),
        };
        if self.allow_methods_with_arguments && shape.has_arguments() {
            return;
        }
        self.finish(node, ctx, &shape, &block, method_name, body_method);
    }

    fn check_super(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let sup = node.as_super_node().expect("kind matched");
        let Some(block_node) = sup.block() else { return };
        let Node::BlockNode { .. } = &block_node else { return };
        let block = block_node.as_block_node().expect("kind matched");
        let Some(body_method) = symbol_proc_body(block.parameters(), block.body()) else {
            return;
        };

        if self.allowed_name(b"super") {
            return;
        }
        let shape = OuterShape {
            opening_paren: sup.lparen_loc().map(|l| l.span()),
            closing_paren: sup.rparen_loc().map(|l| l.span()),
            arguments: sup.arguments(),
        };
        if self.allow_methods_with_arguments && shape.has_arguments() {
            return;
        }
        self.finish(node, ctx, &shape, &block, b"super", body_method);
    }

    fn check_forwarding_super(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let sup = node.as_forwarding_super_node().expect("kind matched");
        let Some(block) = sup.block() else { return };
        let Some(body_method) = symbol_proc_body(block.parameters(), block.body()) else {
            return;
        };

        if self.allowed_name(b"super") {
            return;
        }
        // `zsuper` never has parens or arguments of its own.
        let shape = OuterShape { opening_paren: None, closing_paren: None, arguments: None };
        self.finish(node, ctx, &shape, &block, b"super", body_method);
    }

    /// Shared tail of `check_call`/`check_super`: `AllowComments`, the
    /// offense span, and the fix, once every dispatch-specific exclusion
    /// has already passed.
    fn finish(
        &self,
        node: &Node<'_>,
        ctx: &mut Context<'_>,
        shape: &OuterShape<'_>,
        block: &BlockNode<'_>,
        block_method: &[u8],
        body_method: &[u8],
    ) {
        let block_open = block.opening_loc().span();
        let block_close = block.closing_loc().span();
        if self.allow_comments && Self::contains_comments(ctx, node.span().start, block_close.end) {
            return;
        }
        let offense = Span::new(block_open.start, block_close.end);
        let message = Self::message(body_method, block_method);
        let fix = call_fix(shape, block_open.start, block_close.end, ctx, body_method);
        ctx.report_with_fix(&Self::META, offense, message, fix);
    }
}

impl Rule for SymbolProc {
    const META: RuleMeta = RuleMeta {
        name: "Style/SymbolProc",
        department: Department::Style,
        summary: "Use symbols as procs instead of blocks when possible.",
        explanation: "\
If you prefer a style that allows a block for a method with arguments,
set `true` for `AllowMethodsWithArguments`. `define_method` is allowed by
default; customize with `AllowedMethods`/`AllowedPatterns`.

```ruby
# bad
something.map { |s| s.upcase }
something.map { _1.upcase }
something.map { it.upcase }

# good
something.map(&:upcase)
```

With `AllowMethodsWithArguments: false` (default):

```ruby
# bad
something.do_something(foo) { |o| o.bar }

# good
something.do_something(foo, &:bar)
```

With `AllowComments: true`, a block/lambda with a comment anywhere in its
body is left alone even though it would otherwise be flagged.

With `AllCops: ActiveSupportExtensionsEnabled: true`, `->(x) { x.foo }`,
`proc { |x| x.foo }`, and `Proc.new { |x| x.foo }` are all left alone (their
behavior differs from a symbol-to-proc once ActiveSupport is loaded).

@safety
This cop is unsafe: a `Proc` from `Symbol#to_proc` behaves like a lambda
(strict arity, `ArgumentError` on a wrong argument count) where a `Proc`
from a block does not, and `Symbol#to_proc` cannot call a `protected`
method that would otherwise be accessible.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::SuperNode,
            NodeKind::ForwardingSuperNode,
            NodeKind::LambdaNode,
        ],
        config: &[
            ConfigOption {
                name: "AllowMethodsWithArguments",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Allow the block form for a method call that itself has arguments.",
            },
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&["define_method"]),
                allowed: &[],
                doc: "Method names always allowed to take a block instead of a symbol proc.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method name regex patterns always allowed to take a block.",
            },
            ConfigOption {
                name: "AllowComments",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Allow a block/lambda whose body contains a comment.",
            },
        ],
        blind_spots: "\
`AllowedPatterns` entries that fail to compile as a regex are dropped
(never match) rather than raising a configuration error.
The fix's leading-whitespace trim (RuboCop's
`range_with_surrounding_space(side: :left)`) recognizes a run of spaces/tabs
then a run of `\\n`, matching Unix line endings; it does not special-case a
preceding `\\r` (CRLF sources) or a `\\`-newline continuation.
Cross-cop autocorrection ordering (RuboCop's
`autocorrect_incompatible_with: [Layout::SpaceBeforeBlockBraces]`) is not
replicated; it only matters when both cops run in the same fix pass.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let allowed_patterns =
            options.str_list("AllowedPatterns").iter().filter_map(|p| Regex::new(p).ok()).collect();
        Ok(Self {
            allow_methods_with_arguments: options.bool("AllowMethodsWithArguments"),
            allowed_methods: options.str_list("AllowedMethods"),
            allowed_patterns,
            allow_comments: options.bool("AllowComments"),
            active_support_extensions_enabled: options
                .peer("AllCops", "ActiveSupportExtensionsEnabled")
                .and_then(OptionValue::as_bool)
                .unwrap_or(false),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::LambdaNode { .. } => self.check_lambda(node, ctx),
            Node::CallNode { .. } => self.check_call(node, ctx),
            Node::SuperNode { .. } => self.check_super(node, ctx),
            Node::ForwardingSuperNode { .. } => self.check_forwarding_super(node, ctx),
            _ => {}
        }
    }
}
