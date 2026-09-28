//! `Lint/ErbNewArguments`, ported from RuboCop's
//! `lib/rubocop/cop/lint/erb_new_arguments.rb`.
//!
//! # `minimum_target_ruby_version 2.6`
//!
//! Ruby only started deprecating `ERB.new`'s legacy positional arguments in
//! 2.6, so upstream's `TargetRubyVersion` mixin disables the whole cop below
//! that; [`ErbNewArguments::enter`] mirrors this with a plain
//! `target_ruby_version < 2.6` guard rather than not subscribing at all
//! (there is no static "only run above version X" hook in this engine).
//!
//! # Label-key colon
//!
//! [`ErbNewArguments::build_fix`] reads `trim_mode:`/`eoutvar:` keys out of
//! a trailing kwargs hash and compares them to upstream's
//! `pair.key.source == 'trim_mode'`. Prism keeps the trailing `:` as part of
//! a label-style key's own span (whitequork does not), so
//! [`label_key_text`] strips it before comparing; a hash-rocket-style key
//! (`:trim_mode => x`) never had a trailing colon to strip and so, exactly
//! like upstream, never matches either arm.
//!
//! # Safe navigation
//!
//! Upstream's node pattern uses a bare `send` type, which never matches a
//! `csend` (`&.`) dispatch; [`ErbNewArguments::enter`] guards on
//! `!call.is_safe_navigation()` for the same effect.
//!
//! # The dead `!argument` guard
//!
//! Upstream's `arguments[1..3].each_with_index { |argument, i| next if
//! !argument || ... }` never actually hits the `!argument` half: Ruby's
//! `Array#[]` with a range past the end simply truncates the result instead
//! of padding it with `nil`, and `correct_arguments?` has already returned
//! early for `arguments.size == 1`, so `arguments[1..3]` always yields only
//! real argument nodes. This port therefore just stops at the shorter of
//! `arguments.len()` and 4 positions, with no null check to replicate.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MESSAGE_SAFE_LEVEL: &str = "Passing safe_level with the 2nd argument of `ERB.new` is \
                                   deprecated. Do not use it, and specify other arguments as \
                                   keyword arguments.";

/// `(const {nil? cbase} :ERB)`: a bare `ERB` or top-level-qualified `::ERB`
/// constant, RuboCop's receiver shape for `RESTRICT_ON_SEND = %i[new]`.
fn is_erb_const(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"ERB")
        }
        NodeKind::ConstantPathNode => node.as_constant_path_node().is_some_and(|path| {
            path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == b"ERB")
        }),
        _ => false,
    }
}

/// rubocop-ast's `hash_type?`: a braced hash literal or a braceless
/// bare-keyword-argument list, Prism's two distinct node kinds for it.
fn is_hash_like(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::HashNode | NodeKind::KeywordHashNode)
}

/// `HashNode`/`KeywordHashNode#pairs`: every `AssocNode` element of `node`,
/// excluding an `AssocSplatNode` (`**`) double-splat entry. `node` must
/// already be [`is_hash_like`].
fn hash_pairs<'pr>(node: &Node<'pr>) -> Vec<ruby_ast::node::AssocNode<'pr>> {
    let elements = match node.kind() {
        NodeKind::HashNode => node.as_hash_node().expect("checked").elements(),
        NodeKind::KeywordHashNode => node.as_keyword_hash_node().expect("checked").elements(),
        _ => return Vec::new(),
    };
    elements.iter().filter_map(|el| el.as_assoc_node()).collect()
}

/// `pair.key.source`, with the trailing colon Prism keeps as part of a
/// label-style key's own span (`trim_mode:`) stripped to match whitequork.
/// See the module doc.
fn label_key_text<'a>(ctx: &Context<'a>, key: &Node<'_>) -> &'a [u8] {
    let text = ctx.text(key.span());
    text.strip_suffix(b":").unwrap_or(text)
}

/// `key: value_source`, upstream's `"#{name}: #{pair.value.source}"`/
/// `"#{name}: #{arguments[n].source}"`.
fn build_kwarg(name: &[u8], value_text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(name.len() + 2 + value_text.len());
    out.extend_from_slice(name);
    out.extend_from_slice(b": ");
    out.extend_from_slice(value_text);
    out
}

/// RuboCop's `correct_arguments?`.
fn correct_arguments(arguments: &[Node<'_>]) -> bool {
    arguments.len() == 1 || (arguments.len() == 2 && is_hash_like(&arguments[1]))
}

/// RuboCop's `autocorrect`/`build_kwargs`/`override_by_legacy_args`, folded
/// into one pass: builds the single `Fix` shared by every offense this
/// `ERB.new` call raises (RuboCop's `Corrector#replace` on the same range is
/// idempotent across the multiple `add_offense` blocks that all call
/// `autocorrect` with the same node).
fn build_fix(ctx: &Context<'_>, arguments: &[Node<'_>]) -> Fix {
    let first = arguments[0];
    let last = *arguments.last().expect("checked non-empty by caller");

    let mut trim_mode_kwarg: Option<Vec<u8>> = None;
    let mut eoutvar_kwarg: Option<Vec<u8>> = None;

    // `build_kwargs`: only the trailing kwargs hash (if any) seeds these.
    if is_hash_like(&last) {
        for pair in hash_pairs(&last) {
            let value_text = ctx.text(pair.value().span());
            match label_key_text(ctx, &pair.key()) {
                b"trim_mode" => trim_mode_kwarg = Some(build_kwarg(b"trim_mode", value_text)),
                b"eoutvar" => eoutvar_kwarg = Some(build_kwarg(b"eoutvar", value_text)),
                _ => {}
            }
        }
    }

    // `override_by_legacy_args`: a legacy positional 3rd/4th argument always
    // (3rd) or conditionally (4th, only when it isn't itself the kwargs
    // hash) overrides whatever `build_kwargs` produced.
    if let Some(trim_mode_arg) = arguments.get(2) {
        trim_mode_kwarg = Some(build_kwarg(b"trim_mode", ctx.text(trim_mode_arg.span())));
    }
    if let Some(eoutvar_arg) = arguments.get(3) {
        if !is_hash_like(eoutvar_arg) {
            eoutvar_kwarg = Some(build_kwarg(b"eoutvar", ctx.text(eoutvar_arg.span())));
        }
    }

    let mut good_arguments = ctx.text(first.span()).to_vec();
    for kwarg in [trim_mode_kwarg, eoutvar_kwarg].into_iter().flatten() {
        good_arguments.extend_from_slice(b", ");
        good_arguments.extend_from_slice(&kwarg);
    }

    let range = first.span().join(last.span());
    Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(range, good_arguments)] }
}

/// Emulates Ruby 2.6's deprecation warnings for `ERB.new`'s legacy
/// `safe_level`/`trim_mode`/`eoutvar` positional arguments.
#[derive(Debug, Clone)]
pub struct ErbNewArguments {
    target_ruby_version: f32,
}

impl Rule for ErbNewArguments {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ErbNewArguments",
        department: Department::Lint,
        summary: "Emulates Ruby 2.6's `ERB.new` argument deprecation warnings.",
        explanation: "\
Now non-keyword arguments other than the first one are softly deprecated
and will be removed when Ruby 2.5 becomes EOL. `ERB.new` with non-keyword
arguments is deprecated since ERB 2.2.0. Use `:trim_mode` and `:eoutvar`
keyword arguments to `ERB.new`. This cop identifies places where
`ERB.new(str, trim_mode, eoutvar)` can be replaced by
`ERB.new(str, trim_mode: trim_mode, eoutvar: eoutvar)`.

```ruby
# bad
ERB.new(str, nil, '-', '@output_buffer')

# good
ERB.new(str, trim_mode: '-', eoutvar: '@output_buffer')
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Only a bare `ERB.new`/`::ERB.new` receiver is recognized, matching
upstream's own `(const {nil? cbase} :ERB) :new` node pattern; a receiver
reached through an intermediate constant or variable (e.g. an
`ActionView::Template::Handlers::ERB` alias assigned to a local) is not.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.target_ruby_version < 2.6 {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() || call.name().as_slice() != b"new" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !is_erb_const(&receiver) {
            return;
        }
        let arguments: Vec<Node<'_>> =
            call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
        if arguments.is_empty() || correct_arguments(&arguments) {
            return;
        }

        let mut fix: Option<Fix> = None;
        for (i, message_of) in
            [(1usize, None), (2, Some(b"trim_mode" as &[u8])), (3, Some(b"eoutvar" as &[u8]))]
        {
            let Some(argument) = arguments.get(i) else { break };
            if is_hash_like(argument) {
                continue;
            }
            let span = argument.span();
            let message = match message_of {
                None => MESSAGE_SAFE_LEVEL.to_string(),
                Some(name) => {
                    let name = String::from_utf8_lossy(name);
                    let arg_value = String::from_utf8_lossy(ctx.text(span));
                    let ordinal = if i == 2 { "3rd" } else { "4th" };
                    format!(
                        "Passing {name} with the {ordinal} argument of `ERB.new` is \
                         deprecated. Use keyword argument like `ERB.new(str, {name}: \
                         {arg_value})` instead."
                    )
                }
            };
            let fix = fix.get_or_insert_with(|| build_fix(ctx, &arguments));
            ctx.report_with_fix(&Self::META, span, message, fix.clone());
        }
    }
}
