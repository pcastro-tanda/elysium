//! Per-file rule configuration consumed by [`crate::lint_parsed_with`].
//!
//! A [`FileSettings`] says which rules run for one file and at what
//! (possibly overridden) severity. It is built once per `(config, file)`
//! pair by the caller (typically from a loaded `.rubocop.yml`) and threaded
//! through the engine, which uses it to drop disabled rules' diagnostics and
//! rewrite survivors' severities before returning them.

use std::collections::HashSet;
use std::sync::{Arc, LazyLock};

use parking_lot::Mutex;

use crate::diagnostic::Severity;
use crate::engine::SYNTAX_RULE;

/// One cop's `MessageAnnotator` inputs: `StyleGuide` (already resolved
/// against `StyleGuideBaseURL`), `References`/`Reference`, and `Details`,
/// read from the cop's resolved configuration. Built by the caller (from a
/// loaded `.rubocop.yml`) since this crate never depends on `config`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Annotation {
    /// `StyleGuide`, already joined onto `StyleGuideBaseURL`.
    pub style_guide_url: Option<String>,
    /// `References`/`Reference`, verbatim.
    pub reference_urls: Vec<String>,
    /// `Details`.
    pub details: Option<String>,
}

impl Annotation {
    fn is_empty(&self) -> bool {
        self.style_guide_url.is_none() && self.reference_urls.is_empty() && self.details.is_none()
    }
}

/// `AllCops/DisplayStyleGuide` and `AllCops/ExtraDetails` plus every cop's
/// [`Annotation`]. Shared read-only across every file in a run: unlike
/// [`FileSettings::disable`]/[`FileSettings::set_severity`], none of this
/// varies per file, so callers build it once and hand every [`FileSettings`]
/// an [`Arc`] clone.
#[derive(Debug, Clone, Default)]
pub struct Annotations {
    display_style_guide: bool,
    extra_details: bool,
    cops: Vec<(&'static str, Annotation)>,
}

impl Annotations {
    /// Builds the annotation table from `AllCops`'s two switches; cops are
    /// added with [`Annotations::insert`].
    #[must_use]
    pub fn new(display_style_guide: bool, extra_details: bool) -> Self {
        Self { display_style_guide, extra_details, cops: Vec::new() }
    }

    /// Records `annotation` for `rule`. A no-op when `annotation` names
    /// neither a style guide URL, a reference URL, nor details.
    pub fn insert(&mut self, rule: &'static str, annotation: Annotation) {
        if !annotation.is_empty() {
            self.cops.push((rule, annotation));
        }
    }

    fn get(&self, rule: &str) -> Option<&Annotation> {
        self.cops.iter().find(|(name, _)| *name == rule).map(|(_, a)| a)
    }
}

/// Which rules run, and at what severity, for one file.
///
/// [`SYNTAX_RULE`] can never be disabled: a parse error is always reported
/// regardless of what [`FileSettings::disable`] is called with.
#[derive(Debug, Clone, Default)]
pub struct FileSettings {
    disabled: Vec<&'static str>,
    severities: Vec<(&'static str, Severity)>,
    annotations: Arc<Annotations>,
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

    /// Sets the [`Annotations`] this file's diagnostics are annotated with.
    /// Callers building settings from a [`config::LoadedConfig`] compute
    /// this once per run (it does not vary per file) and pass the same
    /// [`Arc`] to every file.
    pub fn set_annotations(&mut self, annotations: Arc<Annotations>) {
        self.annotations = annotations;
    }

    /// The annotated form of `message` for `rule`, mirroring RuboCop's
    /// `MessageAnnotator#annotate`: `Details` (when `ExtraDetails` applies)
    /// then the joined `StyleGuide`/`References` URLs in parentheses (when
    /// `DisplayStyleGuide` applies). Cop-name prefixing is `MessageAnnotator`
    /// behaviour too, but is the output formatter's job here since it is
    /// unconditional in this engine today. `None` when neither switch
    /// applies or the cop names no URLs/details.
    #[must_use]
    pub(crate) fn annotate(&self, rule: &str, message: &str) -> Option<String> {
        let annotation = self.annotations.get(rule);
        let details = self
            .annotations
            .extra_details
            .then(|| annotation.and_then(|a| a.details.as_deref()))
            .flatten();
        let urls: Vec<&str> = if self.annotations.display_style_guide {
            annotation
                .map(|a| {
                    a.style_guide_url
                        .as_deref()
                        .into_iter()
                        .chain(a.reference_urls.iter().map(String::as_str))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if details.is_none() && urls.is_empty() {
            return None;
        }
        let mut out = String::with_capacity(message.len() + 32);
        out.push_str(message);
        if let Some(details) = details {
            out.push(' ');
            out.push_str(details);
        }
        if !urls.is_empty() {
            out.push_str(" (");
            out.push_str(&urls.join(", "));
            out.push(')');
        }
        Some(out)
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
