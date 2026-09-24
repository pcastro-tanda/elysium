//! `Lint/AmbiguousBlockAssociation`, ported from RuboCop's
//! `lib/rubocop/cop/lint/ambiguous_block_association.rb` plus the
//! `AllowedMethods`/`AllowedPattern` mixins it includes.
//!
//! # Prism shape
//!
//! whitequark represents `foo bar { |x| x.baz }` as a `(send nil :foo (block
//! (send nil :bar) (args (arg :x)) body))`: the block is a *parent* wrapper
//! around the inner `send`, and that wrapper -- not the inner `send` -- is
//! `foo`'s last argument. Prism instead attaches a `{}`/`do...end` block
//! directly to the `CallNode` it belongs to via its own `block` field, so
//! `foo`'s last argument is simply the `bar` `CallNode` itself, with
//! `block()` populated. [`last_argument_call`] recovers RuboCop's
//! `any_block_type?` check against that shape: the last argument must
//! itself be a `CallNode` whose `block()` is a real `BlockNode` (not a
//! `BlockArgumentNode`, Prism's shape for an explicit `&block` pass, which
//! whitequark represents as an ordinary `block_pass` argument and is never
//! `any_block_type?` either) with no arguments of its own.
//!
//! A `CallNode`'s own span always extends through its attached block (there
//! is no separate wrapper node whose span stops short of it, unlike
//! whitequark's inner `send`), so [`call_span_excluding_block`] recomputes
//! RuboCop's `send_node.source` -- used for both the offense message's
//! `%<method>s` placeholder and `AllowedPatterns` matching -- as the call's
//! own span up to its closing paren, last argument, or method name,
//! whichever is furthest right before the block starts.
//!
//! Ruby's own grammar (not this port) already resolves `do...end` vs `{}`
//! block-attachment precedence during parsing: `some_method a do |e| ... end`
//! attaches the block to `some_method` itself (not to `a`), so `a` never
//! looks like a block-taking last argument in the first place and no
//! special-casing of the block's own keyword is needed here.
//!
//! An arrow lambda (`->(x) { }`) is its own `LambdaNode`, never a
//! `CallNode`, so it can never satisfy [`last_argument_call`] to begin
//! with; RuboCop's `lambda_or_proc?` exclusion is therefore only needed
//! here for the bare-method forms `lambda { }`/`proc { }`/`Proc.new { }`
//! ([`is_lambda_or_proc`]), which *are* `CallNode`s with an attached block.
//!
//! `node.assignment?` in RuboCop's `allowed_method_pattern?` is
//! `MethodDispatchNode#assignment?`, aliased to `setter_method?` (`loc?(:operator)`)
//! for a `send`/`csend` node -- true exactly when the call was written with `=`
//! syntax (`recv.attr = val`, `recv[i] = val`), as opposed to `Node#assignment?`'s
//! unrelated `ASSIGNMENTS` set (`lvasgn`/`ivasgn`/etc., which a `send` node is
//! never a member of). This is why RuboCop never flags `self.attr = foo.map { }`:
//! the outer node `on_send` dispatches on is the assignment call itself
//! (`self.attr=`), and `node.assignment?` on it is true.
//! Prism represents such a call as an ordinary `CallNode` with `equal_loc` set to
//! the `=` token's location (`None` for a plain method call), which
//! [`allowed_method_pattern`] checks directly, matching the `CallNode::equal_loc`-based
//! `setter_method?` approximation used elsewhere in this crate (e.g.
//! `Style/SoleNestedConditional`).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop-AST's `OPERATOR_METHODS` (private in `MethodIdentifierPredicates`),
/// which already includes `[]`/`[]=` -- so it alone covers RuboCop's
/// separate `node.method?(:[])` check too.
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

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

/// RuboCop-AST's `lambda_or_proc?` restricted to the `CallNode` shapes that
/// can ever reach it here: `lambda { }`, `proc { }`, `Proc.new { }`, or
/// `::Proc.new { }`.
fn is_lambda_or_proc(call: &CallNode<'_>) -> bool {
    let name = call.name();
    let name = name.as_slice();
    if call.receiver().is_none() && (name == b"lambda" || name == b"proc") {
        return true;
    }
    name == b"new" && call.receiver().is_some_and(|r| is_proc_const(&r))
}

/// The end of `call`'s own source, excluding any attached block: RuboCop's
/// `send_node.source` for a block's associated call, ported since a
/// `CallNode`'s span always extends through its own attached block.
fn call_end_excluding_block(call: &CallNode<'_>) -> u32 {
    if let Some(closing) = call.closing_loc() {
        return closing.span().end;
    }
    if let Some(args) = call.arguments() {
        if let Some(last) = args.arguments().last() {
            return last.span().end;
        }
    }
    call.message_loc().map_or_else(|| call.as_node().span().start, |loc| loc.span().end)
}

/// `call`'s own span, excluding any attached block.
fn call_span_excluding_block(call: &CallNode<'_>) -> Span {
    Span::new(call.as_node().span().start, call_end_excluding_block(call))
}

/// RuboCop's `ambiguous_block_association?`: `node`'s last argument is
/// itself a `CallNode` carrying a real `{}`/`do...end` block (not an
/// explicit `&block` pass) with no arguments of its own.
fn last_argument_call<'pr>(node: &CallNode<'pr>) -> Option<CallNode<'pr>> {
    let args = node.arguments()?;
    let call = args.arguments().last()?.as_call_node()?;
    match call.block()? {
        Node::BlockNode { .. } if call.arguments().is_none() => Some(call),
        _ => None,
    }
}

/// RuboCop's `allowed_method_pattern?`.
fn allowed_method_pattern(
    node: &CallNode<'_>,
    inner: &CallNode<'_>,
    ctx: &Context<'_>,
    allowed_methods: &[String],
    allowed_patterns: &[Regex],
) -> bool {
    if node.equal_loc().is_some() || OPERATOR_METHODS.contains(&node.name().as_slice()) {
        return true;
    }
    let inner_name = inner.name();
    let inner_name = String::from_utf8_lossy(inner_name.as_slice());
    if allowed_methods.iter().any(|m| m == inner_name.as_ref()) {
        return true;
    }
    let text = String::from_utf8_lossy(ctx.text(call_span_excluding_block(inner)));
    allowed_patterns.iter().any(|pattern| pattern.is_match(&text))
}

/// RuboCop's `wrap_in_parentheses`: the gap between the method name and the
/// first argument becomes `(`; a `)` is inserted right after the last
/// argument (here, the block-carrying call's own full span, block
/// included).
fn build_fix(node: &CallNode<'_>, insert_close_at: u32) -> Option<Fix> {
    let message_loc = node.message_loc()?;
    let args = node.arguments()?;
    let first_arg = args.arguments().first()?;
    let gap = Span::new(message_loc.span().end, first_arg.span().start);
    Some(Fix {
        applicability: Applicability::Safe,
        edits: vec![
            Edit::replace(gap, b"(".to_vec()),
            Edit::insert(insert_close_at, b")".to_vec()),
        ],
    })
}

/// Checks for ambiguous block association with method when param passed
/// without parentheses.
#[derive(Debug, Clone)]
pub struct AmbiguousBlockAssociation {
    allowed_methods: Vec<String>,
    allowed_patterns: Vec<Regex>,
}

impl AmbiguousBlockAssociation {
    fn check(&self, node: &CallNode<'_>, ctx: &mut Context<'_>) {
        let Some(inner) = last_argument_call(node) else { return };
        if node.opening_loc().is_some() || is_lambda_or_proc(&inner) {
            return;
        }
        if allowed_method_pattern(node, &inner, ctx, &self.allowed_methods, &self.allowed_patterns)
        {
            return;
        }

        let param_span = inner.as_node().span();
        let method_span = call_span_excluding_block(&inner);
        let param_text = String::from_utf8_lossy(ctx.text(param_span)).into_owned();
        let method_text = String::from_utf8_lossy(ctx.text(method_span)).into_owned();
        let message = format!(
            "Parenthesize the param `{param_text}` to make sure that the block will be \
             associated with the `{method_text}` method call."
        );

        let span = node.as_node().span();
        match build_fix(node, param_span.end) {
            Some(fix) => ctx.report_with_fix(&Self::META, span, message, fix),
            None => ctx.report(&Self::META, span, message),
        }
    }
}

impl Rule for AmbiguousBlockAssociation {
    const META: RuleMeta = RuleMeta {
        name: "Lint/AmbiguousBlockAssociation",
        department: Department::Lint,
        summary: "Checks for ambiguous block association with method when param passed without \
                   parentheses.",
        explanation: "\
This cop can customize allowed methods with `AllowedMethods`. By default,
there are no methods allowed.

```ruby
# bad
some_method a { |val| puts val }

# good
# With parentheses, there's no ambiguity.
some_method(a { |val| puts val })
# or (different meaning)
some_method(a) { |val| puts val }

# good
# Operator methods require no disambiguation
foo == bar { |b| b.baz }

# good
# Lambda arguments require no disambiguation
foo = ->(bar) { bar.baz }
```

With `AllowedMethods: [change]` (default: `[]`):

```ruby
# good
expect { do_something }.to change { object.attribute }
```

With `AllowedPatterns: ['change']` (default: `[]`):

```ruby
# good
expect { do_something }.to change { object.attribute }
expect { do_something }.to not_change { object.attribute }
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method names always allowed to take an ambiguous block without \
                      parentheses.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method name regex patterns always allowed to take an ambiguous block \
                      without parentheses.",
            },
        ],
        blind_spots: "\
The deprecated `IgnoredMethods`/`IgnoredPatterns`/`ExcludedMethods` config-key aliases (superseded
by `AllowedMethods`/`AllowedPatterns` since RuboCop 0.90/1.5) are not read; only the current keys
are. `AllowedPatterns` entries that fail to compile as a Rust regex are dropped (never match)
rather than raising a configuration error.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let allowed_methods = options.str_list("AllowedMethods");
        let allowed_patterns =
            options.str_list("AllowedPatterns").iter().filter_map(|p| Regex::new(p).ok()).collect();
        Ok(Self { allowed_methods, allowed_patterns })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(call) = node.as_call_node() {
            self.check(&call, ctx);
        }
    }
}
