//! The rule contract. Every rule is a struct implementing [`Rule`] whose
//! metadata lives in an associated `META` constant so the registry, the docs
//! generator, and the CLI can read it without instantiating anything.

use ruby_ast::{Node, NodeKind};

use crate::context::Context;
use crate::diagnostic::{Diagnostic, Severity};
use crate::options::{OptionError, RuleOptions};

/// RuboCop departments (plus the ecosystem ones ported later).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum Department {
    Bundler,
    Gemspec,
    Layout,
    Lint,
    Metrics,
    Migration,
    Naming,
    Security,
    Style,
    Rails,
    Performance,
    RSpec,
}

impl Department {
    /// The name as it appears in cop names, e.g. `Style`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bundler => "Bundler",
            Self::Gemspec => "Gemspec",
            Self::Layout => "Layout",
            Self::Lint => "Lint",
            Self::Metrics => "Metrics",
            Self::Migration => "Migration",
            Self::Naming => "Naming",
            Self::Security => "Security",
            Self::Style => "Style",
            Self::Rails => "Rails",
            Self::Performance => "Performance",
            Self::RSpec => "RSpec",
        }
    }
}

/// How far a rule has been validated against real code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Stability {
    /// New; fixtures only. Off unless explicitly enabled.
    Nursery,
    /// Fixtures pass; conformance below the bar or not yet measured.
    Preview,
    /// Above 99% agreement with RuboCop on the conformance corpus.
    Stable,
}

impl Stability {
    /// Lower-case name for CLI output.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nursery => "nursery",
            Self::Preview => "preview",
            Self::Stable => "stable",
        }
    }
}

/// Whether and how safely a rule can fix what it reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixAvailability {
    /// Report only.
    None,
    /// Every fix the rule emits is behaviour-preserving.
    Safe,
    /// Fixes may change behaviour; opt-in.
    Unsafe,
}

/// Default value of a config option, mirroring RuboCop's YAML types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfigDefault {
    /// YAML boolean.
    Bool(bool),
    /// YAML integer.
    Int(i64),
    /// YAML float.
    Float(f64),
    /// YAML string.
    Str(&'static str),
    /// YAML sequence of strings.
    StrList(&'static [&'static str]),
    /// Explicit `~`/absent.
    Nil,
}

/// One entry in a rule's config schema, using RuboCop's option names.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfigOption {
    /// RuboCop option name, e.g. `EnforcedStyle`.
    pub name: &'static str,
    /// Default matching RuboCop's `config/default.yml`.
    pub default: ConfigDefault,
    /// Allowed values for enum-like options (`SupportedStyles`), if any.
    pub allowed: &'static [&'static str],
    /// One-line description for docs.
    pub doc: &'static str,
}

/// Static description of a rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuleMeta {
    /// RuboCop cop name, e.g. `Style/FrozenStringLiteralComment`.
    pub name: &'static str,
    /// Department the rule belongs to.
    pub department: Department,
    /// One-line summary.
    pub summary: &'static str,
    /// Long explanation in Markdown, including good and bad examples.
    pub explanation: &'static str,
    /// Enabled in RuboCop's defaults.
    pub enabled_by_default: bool,
    /// RuboCop's default severity for this department or cop.
    pub severity: Severity,
    /// Fix availability.
    pub fix: FixAvailability,
    /// Validation status.
    pub stability: Stability,
    /// Node kinds this rule wants `enter`/`leave` for. Empty for rules that
    /// only use the file hooks (comment- or line-based rules).
    pub kinds: &'static [NodeKind],
    /// Config schema.
    pub config: &'static [ConfigOption],
    /// Known false negatives and heuristic limits, in Markdown.
    pub blind_spots: &'static str,
}

/// A lint rule.
///
/// Instances are configured once per effective configuration and cloned per
/// file, so rules may keep per-file state in `self`. Anything shared across
/// rules (scopes, comments, directives) is reached through [`Context`].
pub trait Rule: Clone + Send + Sync + 'static {
    /// Static metadata.
    const META: RuleMeta;

    /// Builds a configured instance of the rule.
    ///
    /// Returns an [`OptionError`] for an option value the rule cannot
    /// honour, mirroring RuboCop's "unsupported style ... for
    /// `EnforcedStyle`" configuration errors.
    fn configure(options: &RuleOptions) -> Result<Self, OptionError>
    where
        Self: Sized;

    /// Called once before the tree is walked. Not called for files with
    /// syntax errors.
    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let _ = ctx;
    }

    /// Called when entering a node whose kind is listed in `META.kinds`.
    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let _ = (node, ctx);
    }

    /// Called when leaving a node whose kind is listed in `META.kinds`.
    fn leave(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let _ = (node, ctx);
    }

    /// Called once after the walk. Not called for files with syntax errors.
    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let _ = ctx;
    }

    /// Called once per file, after every rule's [`Rule::file_end`] has run,
    /// with every diagnostic reported so far (`reported`): the raw,
    /// unsorted-by-suppression accumulation before directive filtering and
    /// deduplication, sorted by span start. `RedundantCopDisableDirective`
    /// uses this to see the whole team's results and flag disable
    /// directives nothing actually triggered. Not called for files with
    /// syntax errors. Rules may [`Context::report`] from here.
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]) {
        let _ = (ctx, reported);
    }
}

/// Builds the compile-time subscription table a generated dispatcher
/// indexes with `kind as usize` to decide whether a rule wants a node.
#[must_use]
pub const fn subscription_table(kinds: &[NodeKind]) -> [bool; NodeKind::COUNT] {
    let mut table = [false; NodeKind::COUNT];
    let mut i = 0;
    while i < kinds.len() {
        table[kinds[i] as usize] = true;
        i += 1;
    }
    table
}

/// Static dispatch target generated by the rules crate: fans each event out
/// to every enabled rule that subscribed to it.
pub trait Dispatch {
    /// See [`Rule::file_start`].
    fn file_start(&mut self, ctx: &mut Context<'_>);
    /// See [`Rule::enter`]. `kind` is `node.kind()`, computed once by the engine.
    fn enter(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>);
    /// See [`Rule::leave`].
    fn leave(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>);
    /// See [`Rule::file_end`].
    fn file_end(&mut self, ctx: &mut Context<'_>);
    /// See [`Rule::file_finish`].
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]);
}

/// A dispatcher with no rules; useful for parsing-only runs and benchmarks.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoRules;

impl Dispatch for NoRules {
    fn file_start(&mut self, _: &mut Context<'_>) {}
    fn enter(&mut self, _: NodeKind, _: &Node<'_>, _: &mut Context<'_>) {}
    fn leave(&mut self, _: NodeKind, _: &Node<'_>, _: &mut Context<'_>) {}
    fn file_end(&mut self, _: &mut Context<'_>) {}
    fn file_finish(&mut self, _: &mut Context<'_>, _: &[Diagnostic]) {}
}
