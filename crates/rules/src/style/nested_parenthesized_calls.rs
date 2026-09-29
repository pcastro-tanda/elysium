//! `Style/NestedParenthesizedCalls`, ported from RuboCop's
//! `lib/rubocop/cop/style/nested_parenthesized_calls.rb` plus the
//! `AllowedMethods` mixin it includes.
//!
//! # Prism shape
//!
//! Upstream walks `node.each_child_node(:call)`: the *direct* children of a
//! parenthesized `send`/`csend` node (its receiver and its argument list,
//! but never its block) that are themselves `send`/`csend` nodes. Prism has
//! no separate `csend` type -- `CallNode::is_safe_navigation` distinguishes
//! it -- and, unlike whitequark (where a `do...end`/`{...}` block wraps its
//! call in a separate `block` node, so the call itself is never `:call`-typed
//! and `each_child_node(:call)` skips it), Prism keeps a block as a field of
//! the same `CallNode`. So a candidate child with a literal `BlockNode` block
//! is skipped explicitly here; a `&blk` forwarding argument is a
//! `BlockArgumentNode`, not a `BlockNode`, and stays eligible. A call argument
//! nested inside a block body (e.g. `method(block_taker { another_method 1
//! })`) is, either way, never visited from the outer call: it is only a
//! grandchild of `block_taker`'s block, not a child of `method`'s argument
//! list.
//!
//! `obj[1]` has no dedicated read-access node in Prism (only the
//! `Index*WriteNode` family exists, for assignment contexts); a plain index
//! read is a `CallNode` named `[]`, which upstream's `operator_method?`
//! already excludes, so it needs no special-casing here either.
//!
//! # Autocorrect and backslash continuations
//!
//! Upstream's autocorrect computes the leading-space range to replace with
//! `(` via `range_with_surrounding_space(first_arg.begin, side: :left,
//! whitespace: true, continuations: true)`. `Context::with_surrounding_space`
//! has no `continuations` option (the `\`+newline skip RuboCop's
//! `RangeHelp#final_pos` performs between the plain space/tab pass and the
//! bare-newline pass), so [`leading_space_start`] reimplements
//! `final_pos`'s four sequential passes locally for this one call site.

use std::borrow::Cow;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `MethodIdentifierPredicates::OPERATOR_METHODS`.
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

const DEFAULT_ALLOWED_METHODS: &[&str] = &[
    "be",
    "be_a",
    "be_an",
    "be_between",
    "be_falsey",
    "be_kind_of",
    "be_instance_of",
    "be_truthy",
    "be_within",
    "eq",
    "eql",
    "end_with",
    "include",
    "match",
    "raise_error",
    "respond_to",
    "start_with",
];

/// Parenthesize method calls which are nested inside the argument list of another parenthesized method call.
#[derive(Debug, Clone)]
pub struct NestedParenthesizedCalls {
    /// `AllowedMethods`.
    allowed_methods: Vec<String>,
}

impl Rule for NestedParenthesizedCalls {
    const META: RuleMeta = RuleMeta {
        name: "Style/NestedParenthesizedCalls",
        department: Department::Style,
        summary: "Parenthesize method calls which are nested inside the argument list of another parenthesized method call.",
        explanation: "\
Checks for unparenthesized method calls in the argument list of a parenthesized method call.
`be`, `be_a`, `be_an`, `be_between`, `be_falsey`, `be_kind_of`, `be_instance_of`, `be_truthy`,
`be_within`, `eq`, `eql`, `end_with`, `include`, `match`, `raise_error`, `respond_to`, and
`start_with` methods are allowed by default. These are customizable with the `AllowedMethods`
option.

```ruby
# good
method1(method2(arg))

# bad
method1(method2 arg)
```

With `AllowedMethods: [foo]`:

```ruby
# good
method1(foo arg)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "AllowedMethods",
            default: ConfigDefault::StrList(DEFAULT_ALLOWED_METHODS),
            allowed: &[],
            doc: "Method names always allowed to be called unparenthesized as the sole argument \
of a call that itself is the parenthesized call's sole argument.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allowed_methods: options.str_list("AllowedMethods") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if !is_parenthesized(&call, ctx) {
            return;
        }

        let mut children: Vec<Node<'_>> = call.receiver().into_iter().collect();
        if let Some(arguments) = call.arguments() {
            children.extend(arguments.arguments().iter());
        }

        for child in children {
            let Some(nested) = child.as_call_node() else { continue };
            if nested.block().is_some_and(|b| b.as_block_node().is_some()) {
                // A `do...end`/`{...}` block wraps its call in a `block`
                // node in whitequark, not the `send`/`csend` node itself,
                // so `each_child_node(:call)` never yields it. Prism keeps
                // the block as a field of the same `CallNode`, so it must be
                // excluded here explicitly. A `&blk` forwarding argument is
                // a `BlockArgumentNode`, not a `BlockNode`, and stays a
                // plain, eligible `send` either way.
                continue;
            }
            if self.allowed_omission(&nested, &call, ctx) {
                continue;
            }

            let span = nested.location().span();
            let source = String::from_utf8_lossy(ctx.text(span)).into_owned();
            let message: Cow<'static, str> =
                Cow::Owned(format!("Add parentheses to nested method call `{source}`."));

            let fix = autocorrect(&nested, ctx);
            ctx.report_with_fix(&Self::META, span, message, fix);
        }
    }
}

impl NestedParenthesizedCalls {
    /// `#allowed_omission?`.
    fn allowed_omission(
        &self,
        nested: &CallNode<'_>,
        outer: &CallNode<'_>,
        ctx: &Context<'_>,
    ) -> bool {
        !has_arguments(nested)
            || is_parenthesized(nested, ctx)
            || is_setter_method(nested)
            || is_operator_method(nested)
            || self.is_allowed(nested, outer)
    }

    /// `#allowed?`.
    fn is_allowed(&self, nested: &CallNode<'_>, outer: &CallNode<'_>) -> bool {
        outer.arguments().is_some_and(|a| a.arguments().len() == 1)
            && self.allowed_methods.iter().any(|m| m.as_bytes() == nested.name().as_slice())
            && nested.arguments().is_some_and(|a| a.arguments().len() == 1)
    }
}

/// `ParameterizedNode#parenthesized?`: the call's closing delimiter is `)`.
fn is_parenthesized(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    call.closing_loc().is_some_and(|loc| ctx.text(loc.span()) == b")")
}

/// `ParameterizedNode#arguments?`.
fn has_arguments(call: &CallNode<'_>) -> bool {
    call.arguments().is_some_and(|a| !a.arguments().is_empty())
}

/// `MethodDispatchNode#setter_method?`: the call carries an `=` operator
/// location, i.e. was written as `recv.attr = value` rather than
/// `recv.send(:attr=, value)`.
fn is_setter_method(call: &CallNode<'_>) -> bool {
    call.equal_loc().is_some()
}

/// `MethodIdentifierPredicates#operator_method?`.
fn is_operator_method(call: &CallNode<'_>) -> bool {
    OPERATOR_METHODS.contains(&call.name().as_slice())
}

/// `#autocorrect`: replace the whitespace (and any `\`-continued newline)
/// between the nested call's message and its first argument with `(`, then
/// insert `)` right after the last argument.
fn autocorrect(nested: &CallNode<'_>, ctx: &Context<'_>) -> Fix {
    let arguments = nested.arguments().expect("allowed_omission checked arguments?");
    let args = arguments.arguments();
    let first_arg = args.first().expect("has_arguments checked non-empty");
    let last_arg = args.last().expect("has_arguments checked non-empty");

    let first_start = first_arg.span().start;
    let leading_space = Span::new(leading_space_start(ctx, first_start), first_start);

    Fix {
        applicability: Applicability::Safe,
        edits: vec![
            Edit::replace(leading_space, b"(".to_vec()),
            Edit::insert(last_arg.span().end, b")".to_vec()),
        ],
    }
}

/// `RangeHelp#final_pos` stepping left with `continuations: true, newlines:
/// true (default), whitespace: true`: a run of plain spaces/tabs, then a run
/// of `\`+newline continuations, then a run of bare newlines, then a run of
/// any Ruby whitespace byte.
fn leading_space_start(ctx: &Context<'_>, mut pos: u32) -> u32 {
    let bytes = ctx.source().bytes();
    while pos > 0 && matches!(bytes[pos as usize - 1], b' ' | b'\t') {
        pos -= 1;
    }
    while pos >= 2 && &bytes[(pos - 2) as usize..pos as usize] == b"\\\n" {
        pos -= 2;
    }
    while pos > 0 && bytes[pos as usize - 1] == b'\n' {
        pos -= 1;
    }
    while pos > 0 && is_ruby_whitespace(bytes[pos as usize - 1]) {
        pos -= 1;
    }
    pos
}

/// Local copy of `ruby_source::is_ruby_whitespace` (private crate helper).
fn is_ruby_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}
