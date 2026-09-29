//! `Style/RedundantFetchBlock`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_fetch_block.rb`, together with the
//! `SafeForConstants`-gated part of its `FrozenStringLiteral` mixin
//! (`check_for_string?` calls `frozen_string_literals_enabled?` directly,
//! unlike `Style/RedundantFreeze`'s own use of the mixin, so no
//! `target_ruby_version >= 3.0` gate or `uninterpolated_string?` check
//! applies here).
//!
//! Upstream matches only against `BASIC_LITERALS` (`str`, `int`, `float`,
//! `sym`, `true`, `false`, `nil`, `complex`, `rational`, `regopt`) or a
//! constant, keeping composite literals (interpolated strings/symbols,
//! arrays, hashes, ranges, regexps) and multi-statement/parameterized
//! blocks unmatched entirely -- ported as [`is_candidate_body`].

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, CallNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use `fetch(key, value)` instead of `fetch(key) { value }` when value has Numeric, Rational, Complex, Symbol or String type, `false`, `true`, `nil` or is a constant.
#[derive(Debug, Clone)]
pub struct RedundantFetchBlock {
    /// `SafeForConstants`.
    safe_for_constants: bool,
    /// `AllCops: StringLiteralsFrozenByDefault`.
    string_literals_frozen_by_default: bool,
    /// `check_for_string?`: `FrozenStringLiteral#frozen_string_literals_enabled?`,
    /// recomputed per file in [`Rule::file_start`] since it depends on the
    /// file's own magic comments.
    frozen_string_literals_enabled: bool,
}

/// RuboCop's `FrozenStringLiteral#frozen_string_literals_enabled?`: an
/// explicit leading `# frozen_string_literal: <value>` (or
/// `frozen-string-literal`, case-insensitively on both the key and a
/// `true`/`false` value) magic comment wins outright either way; otherwise
/// falls back to `AllCops: StringLiteralsFrozenByDefault`.
fn frozen_string_literals_enabled(
    ctx: &Context<'_>,
    string_literals_frozen_by_default: bool,
) -> bool {
    ctx.parsed()
        .magic_comments()
        .find_map(|comment| {
            let key = comment.key();
            (key.eq_ignore_ascii_case(b"frozen_string_literal")
                || key.eq_ignore_ascii_case(b"frozen-string-literal"))
            .then(|| comment.value().eq_ignore_ascii_case(b"true"))
        })
        .unwrap_or(string_literals_frozen_by_default)
}

/// Whether `block` has no declared parameters at all: upstream's `(args)`
/// node pattern, matched only by a parameterless block (`{ }`/`do end`, with
/// or without an explicit empty `||`/block-local list).
fn has_no_parameters(block: &BlockNode<'_>) -> bool {
    match block.parameters() {
        None => true,
        Some(params) => params
            .as_block_parameters_node()
            .is_some_and(|bp| bp.parameters().is_none() && bp.locals().is_empty()),
    }
}

/// The block's body, classified the way upstream's node pattern implicitly
/// does: `Empty` for `{}` (matches `nil?`), `Single` for a body whose
/// `StatementsNode` holds exactly one statement (matched against
/// `basic_literal?`/`const_type?`), or `Multi` for two or more statements --
/// which the pattern's single-child match position can never match, so
/// `redundant_fetch_block_candidate?` fails and `on_block` never fires at
/// all (crucially, this is *not* the same as an empty block).
enum BlockBody<'pr> {
    Empty,
    Single(Node<'pr>),
    Multi,
}

fn classify_block_body<'pr>(block: &BlockNode<'pr>) -> BlockBody<'pr> {
    let Some(body) = block.body() else { return BlockBody::Empty };
    let Some(stmts) = body.as_statements_node() else { return BlockBody::Multi };
    let list = stmts.body();
    match list.len() {
        1 => BlockBody::Single(list.first().expect("len == 1")),
        _ => BlockBody::Multi,
    }
}

/// rubocop-ast's `BASIC_LITERALS` (`LITERALS - COMPOSITE_LITERALS`): every
/// truthy/falsey literal type except the composite ones (`dstr`, `xstr`,
/// `dsym`, `array`, `hash`, `irange`, `erange`, `regexp`).
fn is_basic_literal(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
    )
}

/// A bare or qualified constant reference (`const_type?`).
fn is_const(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

/// `redundant_fetch_block_candidate?`'s body alternation: `nil? basic_literal?
/// const_type?`. `body` is the block's sole statement, or `None` for an
/// empty block (which always qualifies, matching upstream's `nil?`).
fn is_candidate_body(body: Option<&Node<'_>>) -> bool {
    match body {
        None => true,
        Some(node) => is_basic_literal(node.kind()) || is_const(node.kind()),
    }
}

/// RuboCop's `rails_cache?` node matcher: `(send (const _ :Rails) :cache)`
/// -- `fetch`'s own receiver is itself a call to `.cache` on a bare or
/// namespaced `Rails` constant.
fn is_rails_cache(receiver: Option<&Node<'_>>) -> bool {
    let Some(call) = receiver.and_then(Node::as_call_node) else { return false };
    if call.name().as_slice() != b"cache" {
        return false;
    }
    let Some(recv) = call.receiver() else { return false };
    match recv.kind() {
        NodeKind::ConstantReadNode => {
            recv.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"Rails")
        }
        NodeKind::ConstantPathNode => recv
            .as_constant_path_node()
            .is_some_and(|p| p.name().is_some_and(|n| n.as_slice() == b"Rails")),
        _ => false,
    }
}

impl RedundantFetchBlock {
    /// RuboCop's `should_not_check?`.
    fn should_not_check(&self, call: &CallNode<'_>, body: Option<&Node<'_>>) -> bool {
        let disqualified_by_kind = body.is_some_and(|node| match node.kind() {
            NodeKind::ConstantReadNode | NodeKind::ConstantPathNode => !self.safe_for_constants,
            NodeKind::StringNode => !self.frozen_string_literals_enabled,
            _ => false,
        });
        disqualified_by_kind || is_rails_cache(call.receiver().as_ref())
    }
}

impl Rule for RedundantFetchBlock {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantFetchBlock",
        department: Department::Style,
        summary: "Use `fetch(key, value)` instead of `fetch(key) { value }` when value has Numeric, Rational, Complex, Symbol or String type, `false`, `true`, `nil` or is a constant.",
        explanation: "\
Identifies places where `fetch(key) { value }` can be replaced by
`fetch(key, value)`.

In such cases `fetch(key, value)` method is faster than `fetch(key) { value }`.

NOTE: The block string `'value'` in `hash.fetch(:key) { 'value' }` is
detected when frozen string literal magic comment is enabled (i.e. `#
frozen_string_literal: true`), but not when disabled.

@safety
This cop is unsafe because it cannot be guaranteed that the receiver does
not have a different implementation of `fetch`.

```ruby
# bad
hash.fetch(:key) { 5 }
hash.fetch(:key) { true }
hash.fetch(:key) { nil }
array.fetch(5) { :value }
ENV.fetch(:key) { 'value' }

# good
hash.fetch(:key, 5)
hash.fetch(:key, true)
hash.fetch(:key, nil)
array.fetch(5, :value)
ENV.fetch(:key, 'value')
```

With `SafeForConstants: true`:

```ruby
# bad
ENV.fetch(:key) { VALUE }

# good
ENV.fetch(:key, VALUE)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[linter::ConfigOption {
            name: "SafeForConstants",
            default: linter::ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Whether a constant default value block is also flagged (unsafe: the constant \
                  may be reassigned before the block would have run).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let string_literals_frozen_by_default = options
            .peer("AllCops", "StringLiteralsFrozenByDefault")
            .and_then(linter::OptionValue::as_bool)
            .unwrap_or(false);
        Ok(Self {
            safe_for_constants: options.bool("SafeForConstants"),
            string_literals_frozen_by_default,
            frozen_string_literals_enabled: string_literals_frozen_by_default,
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.frozen_string_literals_enabled =
            frozen_string_literals_enabled(ctx, self.string_literals_frozen_by_default);
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"fetch" {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let args = arguments.arguments();
        if args.len() != 1 {
            return;
        }
        let Some(block) = call.block().as_ref().and_then(Node::as_block_node) else { return };
        if !has_no_parameters(&block) {
            return;
        }
        let body = match classify_block_body(&block) {
            BlockBody::Multi => return,
            BlockBody::Empty => None,
            BlockBody::Single(node) => Some(node),
        };
        if !is_candidate_body(body.as_ref()) || self.should_not_check(&call, body.as_ref()) {
            return;
        }

        let Some(message_loc) = call.message_loc() else { return };
        let span = Span::new(message_loc.span().start, block.as_node().span().end);

        let key = args.first().expect("checked len == 1");
        let key_src = String::from_utf8_lossy(ctx.text(key.span())).into_owned();
        let default_value = match &body {
            Some(node) => String::from_utf8_lossy(ctx.text(node.span())).into_owned(),
            None => "nil".to_owned(),
        };
        let block_src = match &body {
            Some(node) => {
                format!("{{ {} }}", String::from_utf8_lossy(ctx.text(node.span())))
            }
            None => "{}".to_owned(),
        };
        let good = format!("fetch({key_src}, {default_value})");
        let bad = format!("fetch({key_src}) {block_src}");
        let message = format!("Use `{good}` instead of `{bad}`.");

        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(span, good.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }
}
