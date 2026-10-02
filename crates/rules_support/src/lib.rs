//! What every rule crate shares.
//!
//! Each rule crate (`rules` for core RuboCop, one per extension gem)
//! registers its rules with [`rule_set!`], which expands to that crate's
//! `ALL_RULES` and `Slots`. The `registry` crate chains every crate's
//! `Slots` into the one [`SlotList`] its `RuleSet` dispatches to, so all
//! rules still share one walk and no rule is ever behind a `dyn` pointer.
//! Node-kind subscription is a compile-time `const` table per rule.

use std::sync::Arc;

use config::{CopConfig, LoadedConfig, YamlValue};
pub use linter::RuleMeta;
use linter::{
    subscription_table, Context, Diagnostic, GemVersions, OptionError, OptionValue, PeerOptions,
    Rule, RuleOptions,
};
use ruby_ast::{Node, NodeKind};

/// Registers every rule of one rule crate in one place.
///
/// Expands to `pub const ALL_RULES` -- each rule's metadata, in
/// registration order -- and to `pub type Slots`, the [`SlotList`] holding
/// the crate's configured rules. The slot list is a tree of tuples with
/// `Option<Rule>` leaves, so no identifier has to be synthesized for each
/// rule and cop names that share a snake-case file name across departments
/// -- `Layout/LineLength` and `Metrics/LineLength` -- cannot collide. Rules
/// are grouped 32 to a balanced subtree before chaining (eight at the
/// chain's end), keeping type nesting -- and rustc's drop-check/auto-trait
/// recursion over it, which counts against each downstream crate's
/// `recursion_limit` -- at roughly a sixteenth of the rule count. Each
/// link's tail is boxed: `build` and `clone` recurse once per link and
/// return their subtree by value, so an unboxed chain needs stack quadratic
/// in the rule count in unoptimized builds (past ~500 rules it overflowed
/// the test threads' 2 MiB). A crate with no rules yet registers an empty
/// list: `ALL_RULES` is empty and `Slots` is `()`.
#[macro_export]
macro_rules! rule_set {
    () => {
        /// Every rule this crate registers, in registration order.
        pub const ALL_RULES: &[&'static $crate::RuleMeta] = &[];

        /// The configured rules of this crate, as one slot list.
        pub type Slots = ();
    };
    ($($rule:path),+ $(,)?) => {
        /// Every rule this crate registers, in registration order.
        pub const ALL_RULES: &[&'static $crate::RuleMeta] =
            &[$(<$rule as $crate::RuleExt>::META_REF),+];

        /// The configured rules of this crate, as one slot list.
        pub type Slots = $crate::rule_set!(@slots $($rule),+);
    };
    (@slots
        $a0:path, $a1:path, $a2:path, $a3:path, $a4:path, $a5:path, $a6:path, $a7:path,
        $b0:path, $b1:path, $b2:path, $b3:path, $b4:path, $b5:path, $b6:path, $b7:path,
        $c0:path, $c1:path, $c2:path, $c3:path, $c4:path, $c5:path, $c6:path, $c7:path,
        $d0:path, $d1:path, $d2:path, $d3:path, $d4:path, $d5:path, $d6:path, $d7:path,
        $($rest:path),+
    ) => {
        (
            (
                (
                    $crate::rule_set!(@eight $a0, $a1, $a2, $a3, $a4, $a5, $a6, $a7),
                    $crate::rule_set!(@eight $b0, $b1, $b2, $b3, $b4, $b5, $b6, $b7),
                ),
                (
                    $crate::rule_set!(@eight $c0, $c1, $c2, $c3, $c4, $c5, $c6, $c7),
                    $crate::rule_set!(@eight $d0, $d1, $d2, $d3, $d4, $d5, $d6, $d7),
                ),
            ),
            Box<$crate::rule_set!(@slots $($rest),+)>,
        )
    };
    (@slots $a:path, $b:path, $c:path, $d:path, $e:path, $f:path, $g:path, $h:path, $($rest:path),+) => {
        (
            $crate::rule_set!(@eight $a, $b, $c, $d, $e, $f, $g, $h),
            $crate::rule_set!(@slots $($rest),+),
        )
    };
    (@slots $head:path) => { Option<$head> };
    (@slots $head:path, $($tail:path),+) => {
        (Option<$head>, $crate::rule_set!(@slots $($tail),+))
    };
    (@eight $a:path, $b:path, $c:path, $d:path, $e:path, $f:path, $g:path, $h:path) => {
        (
            ((Option<$a>, Option<$b>), (Option<$c>, Option<$d>)),
            ((Option<$e>, Option<$f>), (Option<$g>, Option<$h>)),
        )
    };
}

/// Per-rule constants derived from [`Rule::META`] at compile time.
pub trait RuleExt: Rule {
    /// `META` as a `'static` reference, for `ALL_RULES` and [`RuleOptions`].
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
/// (a disabled peer's options do not apply). The raw `Enabled` value is
/// dropped from the parameter chain -- it would otherwise appear a second
/// time under the same key, shadowed by the resolved flag -- and its one
/// piece of information the flag cannot carry, `Enabled: pending`, is
/// surfaced as `EnabledPending` instead (RuboCop's `Config#for_cop`
/// reporting `'pending'`, which `Lint/RedundantCopDisableDirective`'s
/// `pending_cop_not_run?` needs).
fn cop_options(cop: &CopConfig) -> Vec<(String, OptionValue)> {
    std::iter::once(("Enabled".to_string(), OptionValue::Bool(cop.enabled)))
        .chain(cop.is_pending().then(|| ("EnabledPending".to_string(), OptionValue::Bool(true))))
        .chain(
            cop.options
                .iter()
                .filter(|(key, _)| key.as_str() != "Enabled")
                .map(|(key, value)| (key.clone(), option_value(value))),
        )
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

/// Everything a slot list needs to decide whether a rule runs and with
/// which options.
pub struct Builder<'a> {
    cfg: Option<&'a LoadedConfig>,
    /// When set, exactly these cops run, regardless of `Enabled`, mirroring
    /// RuboCop's `--only`.
    only: Option<&'a [&'a str]>,
    /// The `--only` list handed to rules (see [`RuleOptions::only_run`]); `None`
    /// when `only` did not come from the CLI's `--only`.
    only_run: Option<Arc<[String]>>,
    peers: Arc<PeerOptions>,
    /// The target's locked gem versions (`Config#gem_versions_in_target`).
    gem_versions: Option<Arc<GemVersions>>,
}

impl<'a> Builder<'a> {
    /// Every rule enabled by default, with RuboCop's default options.
    pub fn defaults() -> Self {
        Self {
            cfg: None,
            only: None,
            only_run: None,
            peers: Arc::new(PeerOptions::new()),
            gem_versions: None,
        }
    }

    /// The rules `cfg` enables, configured from it.
    pub fn from_config(cfg: &'a LoadedConfig) -> Self {
        Self {
            cfg: Some(cfg),
            only: None,
            only_run: None,
            peers: Arc::new(peer_options(cfg)),
            gem_versions: cfg.gem_versions().cloned(),
        }
    }

    /// Only `names`, configured from `cfg` and enabled regardless of it;
    /// `only_run` says whether the rules see the run as an `--only` run.
    pub fn restricted(names: &'a [&'a str], cfg: &'a LoadedConfig, only_run: bool) -> Self {
        Self {
            cfg: Some(cfg),
            only: Some(names),
            only_run: only_run.then(|| names.iter().map(|name| (*name).to_string()).collect()),
            peers: Arc::new(peer_options(cfg)),
            gem_versions: cfg.gem_versions().cloned(),
        }
    }

    fn configure<R: Rule>(&self) -> Result<Option<R>, OptionError> {
        let meta = <R as RuleExt>::META_REF;
        let cop = self.cfg.and_then(|cfg| cfg.cop(meta.name));
        let enabled = match (self.only, self.cfg) {
            (Some(names), _) => names.contains(&meta.name),
            // A cop the configuration does not know belongs to an extension
            // gem the project did not load (`plugins:`/`require:`), and
            // RuboCop never registers such a cop.
            (None, Some(_)) => cop.is_some_and(|cop| cop.enabled),
            (None, None) => meta.enabled_by_default,
        };
        if !enabled {
            return Ok(None);
        }
        let own = cop.map(cop_options).unwrap_or_default();
        let options = RuleOptions::new(meta, own, Arc::clone(&self.peers))
            .with_only(self.only_run.clone())
            .with_gem_versions(self.gem_versions.clone());
        R::configure(&options).map(Some)
    }
}

/// One slot list: a tree of tuples whose leaves are optional rules.
pub trait SlotList: Clone + Send + Sync + 'static + Sized {
    /// Configures every rule in the list, leaving disabled ones `None`.
    fn build(builder: &Builder<'_>) -> Result<Self, OptionError>;
    /// Marks every node kind an enabled rule subscribed to.
    fn add_interest(&self, interest: &mut [bool; NodeKind::COUNT]);
    /// [`Rule::file_start`] on every enabled rule.
    fn file_start(&mut self, ctx: &mut Context<'_>);
    /// [`Rule::enter`] on every enabled rule subscribed to `kind`.
    fn enter(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>);
    /// [`Rule::leave`] on every enabled rule subscribed to `kind`.
    fn leave(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>);
    /// [`Rule::file_end`] on every enabled rule.
    fn file_end(&mut self, ctx: &mut Context<'_>);
    /// [`Rule::file_finish`] on every enabled rule.
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]);
}

impl<R: Rule> SlotList for Option<R> {
    fn build(builder: &Builder<'_>) -> Result<Self, OptionError> {
        builder.configure::<R>()
    }

    fn add_interest(&self, interest: &mut [bool; NodeKind::COUNT]) {
        if self.is_some() {
            for (slot, subscribed) in interest.iter_mut().zip(<R as RuleExt>::SUBSCRIBED) {
                *slot |= subscribed;
            }
        }
    }

    #[inline]
    fn file_start(&mut self, ctx: &mut Context<'_>) {
        if let Some(rule) = self {
            rule.file_start(ctx);
        }
    }

    #[inline]
    fn enter(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(rule) = self {
            if <R as RuleExt>::SUBSCRIBED[kind as usize] {
                rule.enter(node, ctx);
            }
        }
    }

    #[inline]
    fn leave(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(rule) = self {
            if <R as RuleExt>::SUBSCRIBED[kind as usize] {
                rule.leave(node, ctx);
            }
        }
    }

    #[inline]
    fn file_end(&mut self, ctx: &mut Context<'_>) {
        if let Some(rule) = self {
            rule.file_end(ctx);
        }
    }

    #[inline]
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]) {
        if let Some(rule) = self {
            rule.file_finish(ctx, reported);
        }
    }
}

impl<A: SlotList, B: SlotList> SlotList for (A, B) {
    fn build(builder: &Builder<'_>) -> Result<Self, OptionError> {
        Ok((A::build(builder)?, B::build(builder)?))
    }

    fn add_interest(&self, interest: &mut [bool; NodeKind::COUNT]) {
        self.0.add_interest(interest);
        self.1.add_interest(interest);
    }

    #[inline]
    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.0.file_start(ctx);
        self.1.file_start(ctx);
    }

    #[inline]
    fn enter(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        self.0.enter(kind, node, ctx);
        self.1.enter(kind, node, ctx);
    }

    #[inline]
    fn leave(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        self.0.leave(kind, node, ctx);
        self.1.leave(kind, node, ctx);
    }

    #[inline]
    fn file_end(&mut self, ctx: &mut Context<'_>) {
        self.0.file_end(ctx);
        self.1.file_end(ctx);
    }

    #[inline]
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]) {
        self.0.file_finish(ctx, reported);
        self.1.file_finish(ctx, reported);
    }
}

/// The slot list of a rule crate that registers no rules.
impl SlotList for () {
    fn build(_builder: &Builder<'_>) -> Result<Self, OptionError> {
        Ok(())
    }

    fn add_interest(&self, _interest: &mut [bool; NodeKind::COUNT]) {}

    #[inline]
    fn file_start(&mut self, _ctx: &mut Context<'_>) {}

    #[inline]
    fn enter(&mut self, _kind: NodeKind, _node: &Node<'_>, _ctx: &mut Context<'_>) {}

    #[inline]
    fn leave(&mut self, _kind: NodeKind, _node: &Node<'_>, _ctx: &mut Context<'_>) {}

    #[inline]
    fn file_end(&mut self, _ctx: &mut Context<'_>) {}

    #[inline]
    fn file_finish(&mut self, _ctx: &mut Context<'_>, _reported: &[Diagnostic]) {}
}

impl<T: SlotList> SlotList for Box<T> {
    fn build(builder: &Builder<'_>) -> Result<Self, OptionError> {
        T::build(builder).map(Box::new)
    }

    fn add_interest(&self, interest: &mut [bool; NodeKind::COUNT]) {
        (**self).add_interest(interest);
    }

    #[inline]
    fn file_start(&mut self, ctx: &mut Context<'_>) {
        (**self).file_start(ctx);
    }

    #[inline]
    fn enter(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        (**self).enter(kind, node, ctx);
    }

    #[inline]
    fn leave(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        (**self).leave(kind, node, ctx);
    }

    #[inline]
    fn file_end(&mut self, ctx: &mut Context<'_>) {
        (**self).file_end(ctx);
    }

    #[inline]
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]) {
        (**self).file_finish(ctx, reported);
    }
}
