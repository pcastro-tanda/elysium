//! Every rule crate composed into the one [`RuleSet`] the binary runs.
//!
//! Each rule crate registers its own rules with `rules_support::rule_set!`;
//! this crate chains their slot lists into [`Slots`] and their metadata
//! into [`ALL_RULES`]. Adding a rule crate is one entry in each.

use config::LoadedConfig;
use linter::{Context, Diagnostic, Dispatch, OptionError, RuleMeta};
use ruby_ast::{Node, NodeKind};
use rules_support::{Builder, SlotList};

/// Every rule crate's slot list, chained: core RuboCop, then one crate per
/// extension gem.
type Slots = (
    rules::Slots,
    (
        rules_rails::Slots,
        (
            rules_performance::Slots,
            (rules_minitest::Slots, (rules_sorbet::Slots, rules_thread_safety::Slots)),
        ),
    ),
);

/// Every crate's `ALL_RULES`, in [`Slots`] order.
const CRATE_RULES: &[&[&RuleMeta]] = &[
    rules::ALL_RULES,
    rules_rails::ALL_RULES,
    rules_performance::ALL_RULES,
    rules_minitest::ALL_RULES,
    rules_sorbet::ALL_RULES,
    rules_thread_safety::ALL_RULES,
];

const RULE_COUNT: usize = {
    let mut count = 0;
    let mut i = 0;
    while i < CRATE_RULES.len() {
        count += CRATE_RULES[i].len();
        i += 1;
    }
    count
};

const ALL_RULES_ARRAY: [&RuleMeta; RULE_COUNT] = {
    let mut all = [CRATE_RULES[0][0]; RULE_COUNT];
    let mut at = 0;
    let mut i = 0;
    while i < CRATE_RULES.len() {
        let mut j = 0;
        while j < CRATE_RULES[i].len() {
            all[at] = CRATE_RULES[i][j];
            at += 1;
            j += 1;
        }
        i += 1;
    }
    all
};

/// Every registered rule's metadata, crate by crate in registration order.
pub const ALL_RULES: &[&RuleMeta] = &ALL_RULES_ARRAY;

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
        Self::from_builder(&Builder::defaults())
            .unwrap_or_else(|err| panic!("rule rejected its own defaults: {err}"))
    }

    /// The rules `cfg` enables, configured from it.
    pub fn from_config(cfg: &LoadedConfig) -> Result<Self, OptionError> {
        Self::from_builder(&Builder::from_config(cfg))
    }

    /// Only `names`, configured from `cfg`, enabled regardless of what
    /// `cfg` says about them (RuboCop's `--only`).
    pub fn only(names: &[&str], cfg: &LoadedConfig) -> Result<Self, OptionError> {
        Self::from_builder(&Builder::restricted(names, cfg, true))
    }

    /// Only `names`, configured from `cfg` and enabled regardless of it, the
    /// way RuboCop's `CopHelper` runs a cop in its specs: unlike
    /// [`RuleSet::only`], the run is not an `--only` run, so rules that read
    /// the registry see every configured cop.
    pub fn isolated(names: &[&str], cfg: &LoadedConfig) -> Result<Self, OptionError> {
        Self::from_builder(&Builder::restricted(names, cfg, false))
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
