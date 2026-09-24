//! Every rule, one file per rule, grouped by RuboCop department.
//!
//! [`rule_set!`] is the single registration point: it expands to the slot
//! list [`RuleSet`] holds and to [`ALL_RULES`]. Node-kind subscription is a
//! compile-time `const` table per rule, and dispatch is a monomorphized
//! walk over the slot list, so no rule is ever behind a `dyn` pointer.

use std::sync::Arc;

use config::{CopConfig, LoadedConfig, YamlValue};
use linter::{
    subscription_table, Context, Diagnostic, Dispatch, OptionError, OptionValue, PeerOptions, Rule,
    RuleMeta, RuleOptions,
};
use ruby_ast::{Node, NodeKind};

pub mod layout;
pub mod lint;
pub mod style;

/// Registers every rule in one place.
///
/// Expands to the private slot-list type [`RuleSet`] stores its configured
/// rules in (a nested `(Option<Rule>, rest)` tuple, so no identifier has to
/// be synthesized for each rule and cop names that share a snake-case file
/// name across departments -- `Layout/LineLength` and `Metrics/LineLength`
/// -- cannot collide) and to [`ALL_RULES`], in registration order.
macro_rules! rule_set {
    ($($rule:path),+ $(,)?) => {
        /// Every registered rule's metadata, in registration order.
        pub const ALL_RULES: &[&'static RuleMeta] = &[$(<$rule as RuleExt>::META_REF),+];

        /// The configured rules [`RuleSet`] dispatches to.
        type Slots = rule_set!(@slots $($rule),+);
    };
    (@slots $head:path) => { (Option<$head>, ()) };
    (@slots $head:path, $($tail:path),+) => { (Option<$head>, rule_set!(@slots $($tail),+)) };
}

rule_set! {
    style::class_and_module_children::ClassAndModuleChildren,
    style::empty_else::EmptyElse,
    style::accessor_grouping::AccessorGrouping,
    style::redundant_regexp_escape::RedundantRegexpEscape,
    style::redundant_regexp_character_class::RedundantRegexpCharacterClass,
    style::numeric_literal_prefix::NumericLiteralPrefix,
    style::if_unless_modifier_of_if_unless::IfUnlessModifierOfIfUnless,
    style::string_concatenation::StringConcatenation,
    lint::redundant_cop_disable_directive::RedundantCopDisableDirective,
    lint::debugger::Debugger,
    lint::duplicate_hash_key::DuplicateHashKey,
    lint::duplicate_methods::DuplicateMethods,
    lint::ambiguous_block_association::AmbiguousBlockAssociation,
    lint::empty_block::EmptyBlock,
    lint::else_layout::ElseLayout,
    lint::redundant_string_coercion::RedundantStringCoercion,
    layout::empty_line_between_defs::EmptyLineBetweenDefs,
    layout::empty_lines_around_class_body::EmptyLinesAroundClassBody,
    layout::space_around_operators::SpaceAroundOperators,
    layout::space_inside_hash_literal_braces::SpaceInsideHashLiteralBraces,
    style::symbol_proc::SymbolProc,
    layout::space_inside_array_literal_brackets::SpaceInsideArrayLiteralBrackets,
    layout::space_inside_block_braces::SpaceInsideBlockBraces,
    layout::extra_spacing::ExtraSpacing,
    layout::hash_alignment::HashAlignment,
    style::word_array::WordArray,
    style::numeric_literals::NumericLiterals,
    style::redundant_parentheses::RedundantParentheses,
    style::redundant_condition::RedundantCondition,
    layout::first_hash_element_indentation::FirstHashElementIndentation,
    layout::first_argument_indentation::FirstArgumentIndentation,
    layout::argument_alignment::ArgumentAlignment,
    style::redundant_return::RedundantReturn,
    style::sole_nested_conditional::SoleNestedConditional,
    layout::line_length::LineLength,
    style::frozen_string_literal_comment::FrozenStringLiteralComment,
    style::string_literals::StringLiterals,
    layout::trailing_empty_lines::TrailingEmptyLines,
    layout::indentation_width::IndentationWidth,
    layout::empty_lines::EmptyLines,
    layout::indentation_consistency::IndentationConsistency,
    style::documentation::Documentation,
    style::guard_clause::GuardClause,
    style::if_unless_modifier::IfUnlessModifier,
    style::hash_syntax::HashSyntax,
    style::mutable_constant::MutableConstant,
    style::trailing_comma_in_arguments::TrailingCommaInArguments,
    style::trailing_comma_in_hash_literal::TrailingCommaInHashLiteral,
    style::trailing_comma_in_array_literal::TrailingCommaInArrayLiteral,
    layout::trailing_whitespace::TrailingWhitespace,
}

/// Per-rule constants derived from [`Rule::META`] at compile time.
trait RuleExt: Rule {
    /// `META` as a `'static` reference, for [`ALL_RULES`] and [`RuleOptions`].
    const META_REF: &'static RuleMeta = &Self::META;
    /// `true` at `kind as usize` for every node kind the rule subscribed to.
    const SUBSCRIBED: [bool; NodeKind::COUNT] = subscription_table(Self::META.kinds);
}

impl<R: Rule> RuleExt for R {}

/// Converts one configuration value to the rule-facing [`OptionValue`].
pub fn option_value(value: &YamlValue) -> OptionValue {
    match value {
        YamlValue::Null => OptionValue::Null,
        YamlValue::Bool(b) => OptionValue::Bool(*b),
        YamlValue::Int(i) => OptionValue::Int(*i),
        YamlValue::Float(f) => OptionValue::Float(*f),
        YamlValue::String(s) | YamlValue::Regexp(s) => OptionValue::Str(s.clone()),
        YamlValue::Array(items) => OptionValue::List(items.iter().map(option_value).collect()),
        YamlValue::Mapping(map) => OptionValue::Map(
            map.iter().map(|(key, value)| (key.to_string(), option_value(value))).collect(),
        ),
    }
}

/// A cop's configured options, in the rule-facing representation, plus its
/// resolved `Enabled` flag so a rule can mirror RuboCop's `for_enabled_cop`
/// (a disabled peer's options do not apply).
fn cop_options(cop: &CopConfig) -> Vec<(String, OptionValue)> {
    std::iter::once(("Enabled".to_string(), OptionValue::Bool(cop.enabled)))
        .chain(cop.options.iter().map(|(key, value)| (key.clone(), option_value(value))))
        .collect()
}

/// Every cop's options plus `AllCops`, so a rule can read another cop's
/// settings or global ones such as `TargetRubyVersion`.
fn peer_options(cfg: &LoadedConfig) -> PeerOptions {
    let mut peers: PeerOptions =
        cfg.cops().map(|(name, cop)| (name.to_string(), cop_options(cop))).collect();
    let all_cops = cfg
        .all_cops()
        .raw()
        .iter()
        .map(|(key, value)| (key.to_string(), option_value(value)))
        .collect();
    peers.insert("AllCops".to_string(), all_cops);
    peers
}

/// Everything the slot list needs to decide whether a rule runs and with
/// which options.
struct Builder<'a> {
    cfg: Option<&'a LoadedConfig>,
    /// When set, exactly these cops run, regardless of `Enabled`, mirroring
    /// RuboCop's `--only`.
    only: Option<&'a [&'a str]>,
    peers: Arc<PeerOptions>,
}

impl Builder<'_> {
    fn configure<R: Rule>(&self) -> Result<Option<R>, OptionError> {
        let meta = <R as RuleExt>::META_REF;
        let cop = self.cfg.and_then(|cfg| cfg.cop(meta.name));
        let enabled = match self.only {
            Some(names) => names.contains(&meta.name),
            None => cop.map_or(meta.enabled_by_default, |cop| cop.enabled),
        };
        if !enabled {
            return Ok(None);
        }
        let own = cop.map(cop_options).unwrap_or_default();
        R::configure(&RuleOptions::new(meta, own, Arc::clone(&self.peers))).map(Some)
    }
}

/// One slot list: a nested tuple of optional rules, terminated by `()`.
trait SlotList: Clone + Send + Sync + 'static + Sized {
    fn build(builder: &Builder<'_>) -> Result<Self, OptionError>;
    fn add_interest(&self, interest: &mut [bool; NodeKind::COUNT]);
    fn file_start(&mut self, ctx: &mut Context<'_>);
    fn enter(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>);
    fn leave(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>);
    fn file_end(&mut self, ctx: &mut Context<'_>);
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]);
}

impl SlotList for () {
    fn build(_builder: &Builder<'_>) -> Result<Self, OptionError> {
        Ok(())
    }
    fn add_interest(&self, _interest: &mut [bool; NodeKind::COUNT]) {}
    fn file_start(&mut self, _ctx: &mut Context<'_>) {}
    fn enter(&mut self, _kind: NodeKind, _node: &Node<'_>, _ctx: &mut Context<'_>) {}
    fn leave(&mut self, _kind: NodeKind, _node: &Node<'_>, _ctx: &mut Context<'_>) {}
    fn file_end(&mut self, _ctx: &mut Context<'_>) {}
    fn file_finish(&mut self, _ctx: &mut Context<'_>, _reported: &[Diagnostic]) {}
}

impl<R: Rule, T: SlotList> SlotList for (Option<R>, T) {
    fn build(builder: &Builder<'_>) -> Result<Self, OptionError> {
        Ok((builder.configure::<R>()?, T::build(builder)?))
    }

    fn add_interest(&self, interest: &mut [bool; NodeKind::COUNT]) {
        if self.0.is_some() {
            for (slot, subscribed) in interest.iter_mut().zip(<R as RuleExt>::SUBSCRIBED) {
                *slot |= subscribed;
            }
        }
        self.1.add_interest(interest);
    }

    #[inline]
    fn file_start(&mut self, ctx: &mut Context<'_>) {
        if let Some(rule) = &mut self.0 {
            rule.file_start(ctx);
        }
        self.1.file_start(ctx);
    }

    #[inline]
    fn enter(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(rule) = &mut self.0 {
            if <R as RuleExt>::SUBSCRIBED[kind as usize] {
                rule.enter(node, ctx);
            }
        }
        self.1.enter(kind, node, ctx);
    }

    #[inline]
    fn leave(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(rule) = &mut self.0 {
            if <R as RuleExt>::SUBSCRIBED[kind as usize] {
                rule.leave(node, ctx);
            }
        }
        self.1.leave(kind, node, ctx);
    }

    #[inline]
    fn file_end(&mut self, ctx: &mut Context<'_>) {
        if let Some(rule) = &mut self.0 {
            rule.file_end(ctx);
        }
        self.1.file_end(ctx);
    }

    #[inline]
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]) {
        if let Some(rule) = &mut self.0 {
            rule.file_finish(ctx, reported);
        }
        self.1.file_finish(ctx, reported);
    }
}

/// The configured set of enabled rules for one effective configuration.
///
/// Cloned per file: rules keep per-file state in `self`.
#[derive(Clone)]
pub struct RuleSet {
    slots: Slots,
    /// `true` at `kind as usize` when any enabled rule subscribed to it.
    interest: [bool; NodeKind::COUNT],
}

impl RuleSet {
    fn from_builder(builder: &Builder<'_>) -> Result<Self, OptionError> {
        let slots = <Slots as SlotList>::build(builder)?;
        let mut interest = [false; NodeKind::COUNT];
        slots.add_interest(&mut interest);
        Ok(Self { slots, interest })
    }

    /// Every rule enabled by default, with RuboCop's default options.
    ///
    /// # Panics
    ///
    /// If a rule rejects its own declared defaults, which is a bug in that
    /// rule's schema.
    pub fn rubocop_defaults() -> Self {
        let builder = Builder { cfg: None, only: None, peers: Arc::new(PeerOptions::new()) };
        Self::from_builder(&builder)
            .unwrap_or_else(|err| panic!("rule rejected its own defaults: {err}"))
    }

    /// The rules `cfg` enables, configured from it.
    pub fn from_config(cfg: &LoadedConfig) -> Result<Self, OptionError> {
        let builder = Builder { cfg: Some(cfg), only: None, peers: Arc::new(peer_options(cfg)) };
        Self::from_builder(&builder)
    }

    /// Only `names`, configured from `cfg`, enabled regardless of what
    /// `cfg` says about them (RuboCop's `--only`).
    pub fn only(names: &[&str], cfg: &LoadedConfig) -> Result<Self, OptionError> {
        let builder =
            Builder { cfg: Some(cfg), only: Some(names), peers: Arc::new(peer_options(cfg)) };
        Self::from_builder(&builder)
    }
}

impl Dispatch for RuleSet {
    #[inline]
    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.slots.file_start(ctx);
    }

    #[inline]
    fn enter(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.interest[kind as usize] {
            return;
        }
        self.slots.enter(kind, node, ctx);
    }

    #[inline]
    fn leave(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.interest[kind as usize] {
            return;
        }
        self.slots.leave(kind, node, ctx);
    }

    #[inline]
    fn file_end(&mut self, ctx: &mut Context<'_>) {
        self.slots.file_end(ctx);
    }

    #[inline]
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]) {
        self.slots.file_finish(ctx, reported);
    }
}
