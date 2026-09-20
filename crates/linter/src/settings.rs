//! Per-file rule configuration consumed by [`crate::lint_parsed_with`].
//!
//! A [`FileSettings`] says which rules run for one file and at what
//! (possibly overridden) severity. It is built once per `(config, file)`
//! pair by the caller (typically from a loaded `.rubocop.yml`) and threaded
//! through the engine, which uses it to drop disabled rules' diagnostics and
//! rewrite survivors' severities before returning them.

use std::collections::HashSet;
use std::sync::LazyLock;

use parking_lot::Mutex;

use crate::diagnostic::Severity;
use crate::engine::SYNTAX_RULE;

/// Which rules run, and at what severity, for one file.
///
/// [`SYNTAX_RULE`] can never be disabled: a parse error is always reported
/// regardless of what [`FileSettings::disable`] is called with.
#[derive(Debug, Clone, Default)]
pub struct FileSettings {
    disabled: Vec<&'static str>,
    severities: Vec<(&'static str, Severity)>,
}

impl FileSettings {
    /// Every rule enabled, at its default severity. This is what
    /// [`crate::lint_parsed`] uses.
    #[must_use]
    pub fn all_enabled() -> Self {
        Self::default()
    }

    /// Disables `rule`'s diagnostics for this file.
    ///
    /// A no-op for [`SYNTAX_RULE`], which cannot be disabled.
    pub fn disable(&mut self, rule: &'static str) {
        if rule != SYNTAX_RULE && !self.disabled.contains(&rule) {
            self.disabled.push(rule);
        }
    }

    /// Overrides the severity `rule`'s diagnostics are reported at.
    pub fn set_severity(&mut self, rule: &'static str, severity: Severity) {
        if let Some(entry) = self.severities.iter_mut().find(|(name, _)| *name == rule) {
            entry.1 = severity;
        } else {
            self.severities.push((rule, severity));
        }
    }

    /// True unless `rule` was disabled via [`FileSettings::disable`].
    #[must_use]
    pub fn is_enabled(&self, rule: &str) -> bool {
        !self.disabled.contains(&rule)
    }

    /// The severity override set for `rule` via [`FileSettings::set_severity`], if any.
    #[must_use]
    pub fn severity_override(&self, rule: &str) -> Option<Severity> {
        self.severities.iter().find(|(name, _)| *name == rule).map(|(_, severity)| *severity)
    }
}

/// Interns `name` as a `&'static str`, leaking a fresh allocation the first
/// time a given name is seen.
///
/// Rule names inside the engine are `&'static str` (usually `RuleMeta::name`
/// literals), but a loaded `.rubocop.yml` only produces owned `String`s.
/// This bridges the two: callers building a [`FileSettings`] from a
/// [`config::LoadedConfig`] intern each cop name once here and reuse the
/// returned reference for [`FileSettings::disable`] and
/// [`FileSettings::set_severity`]. Around 600 distinct cop names exist
/// today, so the one-time leak per name is negligible.
#[must_use]
pub fn intern_rule_name(name: &str) -> &'static str {
    static INTERNED: LazyLock<Mutex<HashSet<&'static str>>> =
        LazyLock::new(|| Mutex::new(HashSet::new()));
    let mut interned = INTERNED.lock();
    if let Some(existing) = interned.get(name) {
        return existing;
    }
    let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
    interned.insert(leaked);
    leaked
}
