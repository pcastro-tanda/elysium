//! `Style/HashSlice`, ported from RuboCop's
//! `lib/rubocop/cop/style/hash_slice.rb` plus the `HashSubset` mixin it
//! shares with `Style/HashExcept` (`lib/rubocop/cop/mixin/hash_subset.rb`).
//!
//! # Node shapes
//!
//! Upstream's `block_with_first_arg_check?` node-pattern matches a block
//! whose two parameters are exactly two plain (non-destructured) required
//! arguments, and whose single-statement body is either `$(send {A|B})` or
//! `(send $(send {A|B}) :!)`, where `A`/`B` require one side of the inner
//! `send` (its receiver or its sole argument) to be literally an `lvar` read
//! of the *first* block parameter. Prism's generic `CallNode` covers every
//! `send`/`csend` shape uniformly, so this port subscribes to `CallNode`
//! directly, downcasts its attached block ([`CallNode::block`] ->
//! [`Node::as_block_node`]), and re-derives the same structural checks by
//! hand ([`match_params`], [`single_statement`], [`match_key_side`]) instead
//! of a node-pattern macro.
//!
//! Crucially, whitequark elides a single-statement `begin`, so upstream's
//! `(send $(send {...}) :!)` pattern requires the `!`'s receiver to be a
//! *literal* `send` node -- an explicitly parenthesized `!(k == 0.0)` has a
//! `begin` node in that slot instead, failing the match entirely. Prism
//! always wraps in a `StatementsNode`/`ParenthesesNode`, so this port
//! reproduces the same selectivity by checking `receiver.as_call_node()`
//! without ever unwrapping a `ParenthesesNode` first.
//!
//! # `semantically_except_method?`
//!
//! [`is_except_semantics`] ports the mixin method of the same name
//! (verbatim boolean structure) that decides whether a given
//! method/negation combination expresses "keep entries NOT in this key set"
//! (`Style::HashExcept`'s territory) rather than "keep entries IN this key
//! set" (this cop's). `HashSlice`'s own `semantically_subset_method?` is
//! simply this result negated (`semantically_slice_method?` upstream).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, CallNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `SUBSET_METHODS`/`ACTIVE_SUPPORT_SUBSET_METHODS`.
fn supported_subset_method(method: &[u8], active_support_extensions_enabled: bool) -> bool {
    matches!(method, b"==" | b"!=" | b"eql?" | b"include?")
        || (active_support_extensions_enabled && matches!(method, b"in?" | b"exclude?"))
}

/// rubocop-ast's `LITERALS` (`Node#literal?`), needed by `except_key_source`'s
/// `key.literal?` check. Copied privately from `hash_each_methods.rs`.
fn is_literal_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::RangeNode
            | NodeKind::RationalNode
            | NodeKind::ImaginaryNode
    )
}

/// RuboCop's `call.first_argument`.
fn first_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    call.arguments().and_then(|args| args.arguments().first())
}

/// `node.lvar_type? && node.name == name`-ish check used by both the
/// structural "one side is the key" match and `using_value_variable?`.
fn is_lvar_named(node: &Node<'_>, name: &[u8]) -> bool {
    node.as_local_variable_read_node().is_some_and(|lv| lv.name().as_slice() == name)
}

/// RuboCop's `deparenthesize`-equivalent single-level unwrap used by
/// `range_include?`: whitequark's `receiver.child_nodes.first while
/// receiver.begin_type?` loop, here over `ParenthesesNode`/`StatementsNode`.
fn deparenthesize(mut node: Node<'_>) -> Node<'_> {
    while let Some(paren) = node.as_parentheses_node() {
        let Some(body) = paren.body() else { break };
        node = match body.as_statements_node() {
            Some(stmts) => match stmts.body().first() {
                Some(first) => first,
                None => break,
            },
            None => body,
        };
    }
    node
}

/// `HashSubset#range_include?`: guards `(1..5).include?(k)` / `k.in?('a'..'z')`.
fn range_include(matched: &CallNode<'_>) -> bool {
    if first_argument(matched).is_some_and(|a| a.kind() == NodeKind::RangeNode) {
        return true;
    }
    matched.receiver().is_some_and(|recv| deparenthesize(recv).kind() == NodeKind::RangeNode)
}

/// `HashSubset#using_value_variable?`: the hash-value block parameter used
/// as the receiver or sole argument of `matched` (e.g. `v.include?(k)`).
fn using_value_variable(matched: &CallNode<'_>, value_name: &[u8]) -> bool {
    matched.receiver().is_some_and(|r| is_lvar_named(&r, value_name))
        || first_argument(matched).is_some_and(|a| is_lvar_named(&a, value_name))
}

/// `AllowedReceivers`-free `HashSubset#block_with_first_arg_check?`'s
/// `(args $(arg _key) $(arg _))`: exactly two plain required parameters, no
/// optional/rest/post/keyword/block parameters (which would make the
/// upstream node-pattern's fixed 2-element `args` sequence fail to match).
fn match_params<'pr>(block: &BlockNode<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
    let block_params = block.parameters()?.as_block_parameters_node()?;
    let parameters = block_params.parameters()?;
    if !parameters.optionals().is_empty()
        || parameters.rest().is_some()
        || !parameters.posts().is_empty()
        || !parameters.keywords().is_empty()
        || parameters.keyword_rest().is_some()
        || parameters.block().is_some()
    {
        return None;
    }
    let requireds = parameters.requireds();
    if requireds.len() != 2 {
        return None;
    }
    let key = requireds.first()?;
    let value = requireds.last()?;
    key.as_required_parameter_node()?;
    value.as_required_parameter_node()?;
    Some((key, value))
}

/// Whitequark elides a single-statement `begin`; Prism's `BlockNode::body`
/// is always a `StatementsNode`. Requires exactly one statement, matching
/// the node-pattern's implicit single-expression body.
fn single_statement<'pr>(block: &BlockNode<'pr>) -> Option<Node<'pr>> {
    let stmts = block.body()?.as_statements_node()?;
    if stmts.body().len() != 1 {
        return None;
    }
    stmts.body().first()
}

/// Strips a literal (non-parenthesized) leading `!`, returning the inner
/// call and whether a `!` was stripped. Mirrors
/// `HashSubset#extract_body_if_negated` combined with the `(send $(send
/// {...}) :!)` alternative of `block_with_first_arg_check?`: the `!`'s
/// receiver must be a *literal* `CallNode`, not e.g. a parenthesized one.
fn strip_negation(body: Node<'_>) -> Option<(CallNode<'_>, bool)> {
    if let Some(bang) = body.as_call_node() {
        if bang.name().as_slice() == b"!" && bang.arguments().is_none() {
            let inner = bang.receiver()?;
            return Some((inner.as_call_node()?, true));
        }
    }
    body.as_call_node().map(|call| (call, false))
}

/// Which side of `matched` (receiver or sole argument) is the literal key
/// variable, per the `{(lvar _key) $_ _ | _ $_ (lvar _key)}` union: the
/// receiver is tried first, as upstream's alternation order does.
enum KeySide {
    Receiver,
    Argument,
}

fn match_key_side(matched: &CallNode<'_>, key_name: &[u8]) -> Option<KeySide> {
    let recv = matched.receiver()?;
    let arg = first_argument(matched)?;
    if matched.arguments()?.arguments().len() != 1 {
        return None;
    }
    if is_lvar_named(&recv, key_name) {
        Some(KeySide::Receiver)
    } else if is_lvar_named(&arg, key_name) {
        Some(KeySide::Argument)
    } else {
        None
    }
}

/// `HashSubset#semantically_except_method?`: whether this method/negation
/// combination expresses "exclude this key set" rather than "keep this key
/// set" (this cop's `semantically_slice_method?` is simply `!` this).
fn is_except_semantics(call_name: &[u8], matched_method: &[u8], negated: bool) -> bool {
    let included = |negated: bool| {
        if negated {
            matched_method == b"exclude?"
        } else {
            matched_method == b"include?" || matched_method == b"in?"
        }
    };
    if call_name == b"reject" {
        matched_method == b"==" || matched_method == b"eql?" || included(negated)
    } else {
        matched_method == b"!=" || included(!negated)
    }
}

/// `HashSubset#decorate_source`, for one element of a percent-literal
/// (`%w`/`%W`/`%i`/`%I`) array.
fn decorate_source(value: &Node<'_>, ctx: &Context<'_>) -> String {
    match value.kind() {
        NodeKind::InterpolatedSymbolNode => {
            format!(":\"{}\"", String::from_utf8_lossy(ctx.text(value.span())))
        }
        NodeKind::InterpolatedStringNode => {
            format!("\"{}\"", String::from_utf8_lossy(ctx.text(value.span())))
        }
        NodeKind::SymbolNode => format!(":{}", String::from_utf8_lossy(ctx.text(value.span()))),
        NodeKind::StringNode => {
            to_single_quoted(value.as_string_node().expect("kind matched").unescaped())
        }
        _ => format!("'{}'", String::from_utf8_lossy(ctx.text(value.span()))),
    }
}

/// `HashSubset#to_single_quoted`.
fn to_single_quoted(bytes: &[u8]) -> String {
    let content = String::from_utf8_lossy(bytes);
    let mut out = String::with_capacity(content.len() + 2);
    out.push('\'');
    for ch in content.chars() {
        if ch == '\'' || ch == '\\' {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('\'');
    out
}

/// `ArrayNode#percent_literal?`: opened with `%w(`/`%W(`/`%i(`/`%I(`.
fn is_percent_array(array: &ruby_ast::node::ArrayNode<'_>) -> bool {
    array.opening_loc().is_some_and(|l| {
        let s = l.as_slice();
        s.starts_with(b"%w") || s.starts_with(b"%W") || s.starts_with(b"%i") || s.starts_with(b"%I")
    })
}

/// `HashSubset#except_key_source`.
fn except_key_source(key: &Node<'_>, ctx: &Context<'_>) -> String {
    if let Some(array) = key.as_array_node() {
        let percent = is_percent_array(&array);
        let parts: Vec<String> = array
            .elements()
            .iter()
            .map(|el| {
                if percent {
                    decorate_source(&el, ctx)
                } else {
                    String::from_utf8_lossy(ctx.text(el.span())).into_owned()
                }
            })
            .collect();
        return parts.join(", ");
    }
    let source = String::from_utf8_lossy(ctx.text(key.span())).into_owned();
    if is_literal_kind(key.kind()) {
        source
    } else {
        format!("*{source}")
    }
}

/// Checks for usages of `Hash#reject`, `Hash#select`, and `Hash#filter` methods that can be replaced with `Hash#slice` method.
#[derive(Debug, Clone)]
pub struct HashSlice {
    /// `AllCops/ActiveSupportExtensionsEnabled`.
    active_support_extensions_enabled: bool,
    /// `minimum_target_ruby_version 2.5`.
    target_ruby_version_ok: bool,
}

impl Rule for HashSlice {
    const META: RuleMeta = RuleMeta {
        name: "Style/HashSlice",
        department: Department::Style,
        summary: "Checks for usages of `Hash#reject`, `Hash#select`, and `Hash#filter` methods that can be replaced with `Hash#slice` method.",
        explanation: "\
This cop should only be enabled on Ruby version 2.5 or higher (`Hash#slice`
was added in Ruby 2.5).

For safe detection, it is limited to commonly used string and symbol
comparisons when using `==` or `!=`.

This cop doesn't check for `Hash#delete_if` and `Hash#keep_if` because they
modify the receiver.

This cop is unsafe because it cannot be guaranteed that the receiver is a
`Hash` or responds to the replacement method.

Additionally, the replacement may change the order of the resulting hash:
`Hash#slice` returns entries in the order the keys are given, whereas
`select`, `filter`, and `reject` preserve the entry order of the receiver.

```ruby
# bad
{foo: 1, bar: 2, baz: 3}.select {|k, v| k == :bar }
{foo: 1, bar: 2, baz: 3}.reject {|k, v| k != :bar }
{foo: 1, bar: 2, baz: 3}.filter {|k, v| k == :bar }
{foo: 1, bar: 2, baz: 3}.select {|k, v| k.eql?(:bar) }

# bad
{foo: 1, bar: 2, baz: 3}.select {|k, v| %i[bar].include?(k) }
{foo: 1, bar: 2, baz: 3}.reject {|k, v| !%i[bar].include?(k) }
{foo: 1, bar: 2, baz: 3}.filter {|k, v| %i[bar].include?(k) }

# good
{foo: 1, bar: 2, baz: 3}.slice(:bar)
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let active_support_extensions_enabled = options
            .peer("AllCops", "ActiveSupportExtensionsEnabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(false);
        Ok(Self {
            active_support_extensions_enabled,
            target_ruby_version_ok: options.target_ruby_version() >= 2.5,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.target_ruby_version_ok {
            return;
        }
        let call = node.as_call_node().expect("kind matched");
        if !matches!(call.name().as_slice(), b"select" | b"reject" | b"filter") {
            return;
        }
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        let Some((key, value)) = match_params(&block) else { return };
        let Some(body) = single_statement(&block) else { return };
        let Some((matched, negated)) = strip_negation(body) else { return };

        let key_name = key.as_required_parameter_node().expect("checked").name().as_slice();
        let value_name = value.as_required_parameter_node().expect("checked").name().as_slice();

        if match_key_side(&matched, key_name).is_none() {
            return;
        }

        let method = matched.name().as_slice();
        if !supported_subset_method(method, self.active_support_extensions_enabled) {
            return;
        }
        if range_include(&matched) {
            return;
        }

        let recv = matched.receiver().expect("checked in match_key_side");
        let arg = first_argument(&matched).expect("checked in match_key_side");
        match method {
            b"include?" | b"exclude?" => {
                if using_value_variable(&matched, value_name) {
                    return;
                }
                if ctx.text(arg.span()) != key_name {
                    return;
                }
            }
            b"in?" => {
                if using_value_variable(&matched, value_name) {
                    return;
                }
                if ctx.text(recv.span()) != key_name {
                    return;
                }
            }
            _ => {}
        }

        // `HashSubset#except_key`: the non-key side of the comparison.
        let except_key = if ctx.text(recv.span()) == key_name { arg } else { recv };

        // `HashSubset#safe_to_register_offense?`: only a literal `sym`/`str`
        // is allowed as the except-key for a direct (non-negated) `==`/`!=`.
        if !negated
            && matches!(method, b"==" | b"!=")
            && !matches!(except_key.kind(), NodeKind::SymbolNode | NodeKind::StringNode)
        {
            return;
        }

        if is_except_semantics(call.name().as_slice(), method, negated) {
            return;
        }

        let key_source = except_key_source(&except_key, ctx);
        let preferred = format!("slice({key_source})");
        let message = format!("Use `{preferred}` instead.");

        let Some(message_loc) = call.message_loc() else { return };
        let span = Span::new(message_loc.span().start, block.closing_loc().span().end);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, preferred.into_bytes())],
            },
        );
    }
}
