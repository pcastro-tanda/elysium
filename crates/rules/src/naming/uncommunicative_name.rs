//! Shared checks ported from RuboCop's `UncommunicativeName` mixin
//! (`lib/rubocop/cop/mixin/uncommunicative_name.rb`), used by
//! `Naming/BlockParameterName` and `Naming/MethodParameterName`.
//!
//! # Parameter list traversal
//!
//! Upstream walks `node.arguments` (whitequark's flat, source-ordered
//! child list of the argument node) and reads `arg.children.first` for
//! each entry. Prism instead buckets a parameter list into typed fields
//! (`requireds`, `optionals`, `rest`, `posts`, `keywords`, `keyword_rest`,
//! `block`) on `ParametersNode`. Ruby's grammar fixes that exact sequence
//! as the only legal parameter order, so walking the buckets in
//! `requireds, optionals, rest, posts, keywords, keyword_rest, block`
//! order reproduces upstream's source order for free.
//!
//! A destructured block parameter (`|(a, b), *c|`) is Prism's
//! `MultiTargetNode`. Upstream's generic `check` reads
//! `arg.children.first.to_s` for every parameter node, and for an mlhs
//! (`args (mlhs ...))`) node that reads the mlhs's own first child -- an
//! AST node, not a name -- so the resulting `full_name`/comparisons never
//! meaningfully match anything and no offense is ever issued (verified
//! against RuboCop 1.82.1: `foo { |(a, b)| }` is never flagged). This port
//! reproduces that by skipping `MultiTargetNode` entirely in
//! [`visit_param`] rather than inventing a per-leaf check upstream itself
//! can't perform.
//!
//! `...` (argument forwarding) becomes a `ForwardingParameterNode`
//! (occupying the `rest` bucket), which carries no name and so is
//! silently skipped by [`named_extent`] -- matching upstream, where the
//! equivalent node has no `children.first` either.

use linter::{Context, RuleMeta};
use ruby_ast::node::ParametersNode;
use ruby_ast::{LocationExt as _, Node};
use ruby_source::Span;

/// Which node kind's parameters are being checked, controlling message
/// wording (upstream's `name_type`, based on `node.type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NameType {
    /// `Naming/BlockParameterName`.
    BlockParameter,
    /// `Naming/MethodParameterName`.
    MethodParameter,
}

impl NameType {
    const fn label(self) -> &'static str {
        match self {
            Self::BlockParameter => "block parameter",
            Self::MethodParameter => "method parameter",
        }
    }

    const fn label_capitalized(self) -> &'static str {
        match self {
            Self::BlockParameter => "Block parameter",
            Self::MethodParameter => "Method parameter",
        }
    }
}

/// The four cop-config keys upstream's mixin reads (`MinNameLength`,
/// `AllowNamesEndingInNumbers`, `AllowedNames`, `ForbiddenNames`),
/// resolved once per configured rule instance.
#[derive(Debug, Clone)]
pub(crate) struct UncommunicativeNameConfig {
    pub(crate) min_length: i64,
    pub(crate) allow_names_ending_in_numbers: bool,
    pub(crate) allowed_names: Vec<String>,
    pub(crate) forbidden_names: Vec<String>,
}

/// RuboCop's `check`: walks every parameter reachable from `params` and
/// applies [`issue_offenses`] to each one not exempted.
pub(crate) fn check(
    config: &UncommunicativeNameConfig,
    name_type: NameType,
    params: &ParametersNode<'_>,
    ctx: &mut Context<'_>,
    meta: &RuleMeta,
) {
    for required in &params.requireds() {
        visit_param(config, name_type, &required, ctx, meta);
    }
    for optional in &params.optionals() {
        visit_param(config, name_type, &optional, ctx, meta);
    }
    if let Some(rest) = params.rest() {
        visit_param(config, name_type, &rest, ctx, meta);
    }
    for post in &params.posts() {
        visit_param(config, name_type, &post, ctx, meta);
    }
    for keyword in &params.keywords() {
        visit_param(config, name_type, &keyword, ctx, meta);
    }
    if let Some(keyword_rest) = params.keyword_rest() {
        visit_param(config, name_type, &keyword_rest, ctx, meta);
    }
    if let Some(block) = params.block() {
        visit_param(config, name_type, &block.as_node(), ctx, meta);
    }
}

/// Dispatches one parameter-list entry: skips a destructured target
/// entirely (see the module doc), otherwise extracts a name/range via
/// [`named_extent`] and runs upstream's `check` body on it (the `_`
/// exemption, the leading-underscore trim, `AllowedNames`, then
/// [`issue_offenses`]).
fn visit_param(
    config: &UncommunicativeNameConfig,
    name_type: NameType,
    node: &Node<'_>,
    ctx: &mut Context<'_>,
    meta: &RuleMeta,
) {
    if node.as_multi_target_node().is_some() {
        return;
    }

    let Some((full_name, begin, extra)) = named_extent(node) else { return };
    // Upstream: `next if full_name == '_'`.
    if full_name == b"_" {
        return;
    }
    // Upstream: `name = full_name.gsub(/\A(_+)/, '')`.
    let name = trim_leading_underscores(full_name);
    // Upstream: `next if allowed_names.include?(name)`.
    if config.allowed_names.iter().any(|allowed| allowed.as_bytes() == name) {
        return;
    }
    let length = full_name.len() + extra;
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a parameter name's byte length never approaches u32::MAX"
    )]
    let span = Span::new(begin, begin + length as u32);
    issue_offenses(config, name_type, name, span, ctx, meta);
}

/// Upstream's `name_child = arg.children.first` plus the
/// `length += 1 if arg.restarg_type?` / `length += 2 if
/// arg.kwrestarg_type?` range widening, resolved per Prism parameter node
/// kind: `(name bytes, range start, extra width beyond the name itself)`.
/// `None` for an anonymous `*`/`**`/`&` (no `children.first` upstream
/// either) and for any node kind carrying no name (destructuring is
/// handled by the caller before this is reached).
fn named_extent<'pr>(node: &Node<'pr>) -> Option<(&'pr [u8], u32, usize)> {
    if let Some(p) = node.as_required_parameter_node() {
        return Some((p.name().as_slice(), p.location().span().start, 0));
    }
    if let Some(p) = node.as_optional_parameter_node() {
        return Some((p.name().as_slice(), p.location().span().start, 0));
    }
    if let Some(p) = node.as_rest_parameter_node() {
        return Some((p.name()?.as_slice(), p.location().span().start, 1));
    }
    if let Some(p) = node.as_required_keyword_parameter_node() {
        return Some((p.name().as_slice(), p.location().span().start, 0));
    }
    if let Some(p) = node.as_optional_keyword_parameter_node() {
        return Some((p.name().as_slice(), p.location().span().start, 0));
    }
    if let Some(p) = node.as_keyword_rest_parameter_node() {
        return Some((p.name()?.as_slice(), p.location().span().start, 2));
    }
    if let Some(p) = node.as_block_parameter_node() {
        return Some((p.name()?.as_slice(), p.location().span().start, 0));
    }
    None
}

/// Upstream's `full_name.gsub(/\A(_+)/, '')`: strips a leading run of
/// underscores only (an underscore elsewhere in the name is untouched).
fn trim_leading_underscores(full_name: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < full_name.len() && full_name[i] == b'_' {
        i += 1;
    }
    &full_name[i..]
}

/// Upstream's `issue_offenses`: forbidden-name, then case, then length,
/// then (unless `AllowNamesEndingInNumbers`) trailing-number, each
/// independently evaluated against `name` -- but the engine dedups same-rule
/// diagnostics sharing an identical span (this rule always reports the same
/// `span` for a given parameter), so only the first matching check actually
/// surfaces a diagnostic for it.
fn issue_offenses(
    config: &UncommunicativeNameConfig,
    name_type: NameType,
    name: &[u8],
    span: Span,
    ctx: &mut Context<'_>,
    meta: &RuleMeta,
) {
    let name_str = String::from_utf8_lossy(name);
    if config.forbidden_names.iter().any(|forbidden| forbidden.as_bytes() == name) {
        ctx.report(
            meta,
            span,
            format!("Do not use {name_str} as a name for a {}.", name_type.label()),
        );
    }
    if name_str.chars().any(char::is_uppercase) {
        ctx.report(meta, span, format!("Only use lowercase characters for {}.", name_type.label()));
    }
    let min_length = usize::try_from(config.min_length).unwrap_or(0);
    if name_str.chars().count() < min_length {
        ctx.report(
            meta,
            span,
            format!(
                "{} must be at least {} characters long.",
                name_type.label_capitalized(),
                config.min_length
            ),
        );
    }
    if !config.allow_names_ending_in_numbers
        && name_str.chars().next_back().is_some_and(|c| c.is_ascii_digit())
    {
        ctx.report(meta, span, format!("Do not end {} with a number.", name_type.label()));
    }
}
