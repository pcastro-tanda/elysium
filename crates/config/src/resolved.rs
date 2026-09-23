//! The resolved configuration: `AllCops`, per-cop settings, file matching,
//! `--show-cops` rendering and a stable hash for the cache.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use linter::Severity;

use crate::defaults::DEFAULT_CONFIG;
use crate::file_matcher::FileMatcher;
use crate::obsoletion::RULES;
use crate::yaml::{emit_mapping, Mapping, YamlValue};

/// Extension gems whose default configuration is not embedded yet; the CLI
/// warns when a config asks for them.
const KNOWN_EXTENSIONS: &[&str] = &[
    "rubocop-rails",
    "rubocop-performance",
    "rubocop-rspec",
    "rubocop-rspec_rails",
    "rubocop-rake",
    "rubocop-capybara",
    "rubocop-factory_bot",
    "rubocop-minitest",
    "rubocop-graphql",
    "rubocop-thread_safety",
    "rubocop-sequel",
    "rubocop-i18n",
    "rubocop-rubycw",
    "rubocop-md",
];

/// `AllCops/NewCops`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewCops {
    /// Cops marked `Enabled: pending` run.
    Enable,
    /// Pending cops stay off.
    Disable,
    /// RuboCop's default: pending cops stay off but are reported.
    Pending,
}

impl NewCops {
    fn from_value(value: Option<&YamlValue>) -> Self {
        match value.and_then(YamlValue::as_str) {
            Some("enable") => NewCops::Enable,
            Some("disable") => NewCops::Disable,
            _ => NewCops::Pending,
        }
    }
}

/// The resolved `AllCops` section.
///
/// The booleans mirror RuboCop's `AllCops` switches one for one; collapsing
/// them into enums would only obscure the mapping.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct AllCops {
    /// `TargetRubyVersion`, when the configuration states one.
    pub target_ruby_version: Option<f32>,
    /// `TargetRailsVersion`, when the configuration states one.
    pub target_rails_version: Option<f32>,
    /// `Include` patterns.
    pub include: Vec<String>,
    /// `Exclude` patterns (absolute, as RuboCop rewrites them).
    pub exclude: Vec<String>,
    /// `NewCops`.
    pub new_cops: NewCops,
    /// `DisabledByDefault`.
    pub disabled_by_default: bool,
    /// `EnabledByDefault`.
    pub enabled_by_default: bool,
    /// `SuggestExtensions`.
    pub suggest_extensions: bool,
    /// `ParserEngine` (`parser_whitequark` or `parser_prism`).
    pub parser_engine: String,
    /// `StyleGuideBaseURL`.
    pub style_guide_base_url: Option<String>,
    /// `ActiveSupportExtensionsEnabled`.
    pub active_support_extensions_enabled: bool,
    /// `StringLiteralsFrozenByDefault`.
    pub string_literals_frozen_by_default: bool,
    /// `DisplayCopNames`.
    pub display_cop_names: bool,
    raw: Mapping,
}

impl AllCops {
    fn from_raw(raw: Mapping) -> Self {
        Self {
            target_ruby_version: raw.get("TargetRubyVersion").and_then(YamlValue::as_f32),
            target_rails_version: raw.get("TargetRailsVersion").and_then(YamlValue::as_f32),
            include: raw.get_string_list("Include"),
            exclude: raw.get_string_list("Exclude"),
            new_cops: NewCops::from_value(raw.get("NewCops")),
            disabled_by_default: raw.get("DisabledByDefault").is_some_and(YamlValue::is_truthy),
            enabled_by_default: raw.get("EnabledByDefault").is_some_and(YamlValue::is_truthy),
            suggest_extensions: raw.get("SuggestExtensions").is_some_and(YamlValue::is_truthy),
            parser_engine: raw.get_str("ParserEngine").unwrap_or("parser_whitequark").to_string(),
            style_guide_base_url: raw.get_str("StyleGuideBaseURL").map(str::to_string),
            active_support_extensions_enabled: raw
                .get("ActiveSupportExtensionsEnabled")
                .is_some_and(YamlValue::is_truthy),
            string_literals_frozen_by_default: raw
                .get("StringLiteralsFrozenByDefault")
                .is_some_and(YamlValue::is_truthy),
            display_cop_names: raw.get("DisplayCopNames").is_some_and(YamlValue::is_truthy),
            raw,
        }
    }

    /// Every `AllCops` key, including ones this struct does not name.
    pub fn raw(&self) -> &Mapping {
        &self.raw
    }
}

/// The resolved configuration of one cop.
#[derive(Debug, Clone)]
pub struct CopConfig {
    /// Whether the cop runs, with `pending` resolved through `NewCops`.
    pub enabled: bool,
    /// `Severity`, when the configuration overrides the cop's default.
    pub severity: Option<Severity>,
    /// `Include` patterns.
    pub include: Vec<String>,
    /// `Exclude` patterns (absolute).
    pub exclude: Vec<String>,
    /// Every other parameter, raw.
    pub options: BTreeMap<String, YamlValue>,
    /// The configuration file that mentions this cop, or `None` when the
    /// settings come only from the bundled defaults.
    pub source: Option<PathBuf>,
    /// `Enabled` exactly as RuboCop's `Config#for_cop` reports it, so
    /// `--show-cops` can print `pending`.
    enabled_value: YamlValue,
    raw: Mapping,
}

impl CopConfig {
    /// True when `Enabled: pending` survived resolution.
    pub fn is_pending(&self) -> bool {
        self.enabled_value.as_str() == Some("pending")
    }

    /// Every parameter, in `config/default.yml` order, as `--show-cops`
    /// prints them.
    pub fn raw(&self) -> &Mapping {
        &self.raw
    }
}

/// A fully resolved configuration.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    raw: Mapping,
    loaded_path: Option<PathBuf>,
    root: PathBuf,
    all_cops: AllCops,
    cops: BTreeMap<String, CopConfig>,
    matcher: FileMatcher,
    cop_matchers: BTreeMap<String, FileMatcher>,
    extensions: Vec<String>,
    /// The subset of `extensions` whose gem was found on disk, whether or
    /// not it shipped a `config/default.yml` (`ConfigLoader::merge_with_default`
    /// already merged the latter in).
    resolved_extensions: BTreeSet<String>,
    warnings: Vec<String>,
}

impl LoadedConfig {
    /// Builds the resolved view of a merged configuration hash.
    pub(crate) fn new(
        raw: Mapping,
        loaded_path: Option<PathBuf>,
        root: PathBuf,
        extensions: Vec<String>,
        resolved_extensions: Vec<String>,
        warnings: Vec<String>,
    ) -> Self {
        let all_cops = AllCops::from_raw(raw.get_mapping("AllCops").cloned().unwrap_or_default());
        let configured: BTreeSet<String> = raw
            .keys()
            .filter(|key| !DEFAULT_CONFIG.contains_key(key))
            .map(str::to_string)
            .collect();

        let departments: BTreeSet<String> = raw
            .keys()
            .filter_map(|key| key.rsplit_once('/').map(|(dept, _)| dept.to_string()))
            .collect();

        let mut cops = BTreeMap::new();
        let mut cop_matchers = BTreeMap::new();
        for (name, value) in raw.iter() {
            if !name.contains('/') || departments.contains(name) {
                continue;
            }
            let Some(params) = value.as_mapping() else { continue };
            let mut params = params.clone();
            // `Config#for_cop` merges configuration written under a cop's old
            // name on top of the new one.
            for old_name in RULES.deprecated_names_for(name) {
                if let Some(old) = raw.get_mapping(old_name) {
                    params = params.merged_with(old);
                }
            }
            let enabled_value = enabled_value(name, &params, &raw, all_cops.disabled_by_default);
            let enabled = match &enabled_value {
                YamlValue::Bool(b) => *b,
                YamlValue::String(s) if s == "pending" => all_cops.new_cops == NewCops::Enable,
                other => other.is_truthy(),
            };
            params.insert("Enabled", enabled_value.clone());

            let include = params.get_string_list("Include");
            let exclude = params.get_string_list("Exclude");
            let has_include = params.contains_key("Include");
            let has_exclude = params.contains_key("Exclude");
            if has_include || has_exclude {
                if let Ok(matcher) = FileMatcher::rooted(
                    root.clone(),
                    has_include.then_some(include.as_slice()),
                    has_exclude.then_some(exclude.as_slice()),
                ) {
                    cop_matchers.insert(name.to_string(), matcher);
                }
            }

            let options = params
                .iter()
                .map(|(key, value)| (key.to_string(), value.clone()))
                .collect::<BTreeMap<_, _>>();

            cops.insert(
                name.to_string(),
                CopConfig {
                    enabled,
                    severity: params.get_str("Severity").and_then(Severity::from_name),
                    include,
                    exclude,
                    options,
                    source: if configured.contains(name)
                        || loaded_path.as_ref().is_some_and(|_| {
                            // A cop whose defaults were overridden keeps the
                            // loaded path as its source.
                            DEFAULT_CONFIG.get_mapping(name) != Some(&params)
                        }) {
                        loaded_path.clone()
                    } else {
                        None
                    },
                    enabled_value,
                    raw: params,
                },
            );
        }

        backfill_unconfigured_cops(
            &raw,
            loaded_path.as_deref(),
            all_cops.disabled_by_default,
            &mut cops,
        );

        let matcher =
            FileMatcher::rooted(root.clone(), Some(&all_cops.include), Some(&all_cops.exclude))
                .unwrap_or_else(|_| FileMatcher::rubocop_defaults());

        Self {
            raw,
            loaded_path,
            root,
            all_cops,
            cops,
            matcher,
            cop_matchers,
            extensions,
            resolved_extensions: resolved_extensions.into_iter().collect(),
            warnings,
        }
    }

    /// The directory paths in the configuration are relative to: the directory
    /// of the loaded `.rubocop.yml`, or the working directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The file this configuration was loaded from, if any.
    pub fn loaded_path(&self) -> Option<&Path> {
        self.loaded_path.as_deref()
    }

    /// The resolved `AllCops` section.
    pub fn all_cops(&self) -> &AllCops {
        &self.all_cops
    }

    /// The configuration of one cop, or `None` for an unknown cop.
    pub fn cop(&self, name: &str) -> Option<&CopConfig> {
        self.cops.get(name)
    }

    /// Every known cop, sorted by name.
    pub fn cops(&self) -> impl Iterator<Item = (&str, &CopConfig)> {
        self.cops.iter().map(|(name, config)| (name.as_str(), config))
    }

    /// The department-level settings (`Style: {Enabled: false}`), if any.
    pub fn department(&self, name: &str) -> Option<&Mapping> {
        self.raw.get_mapping(name)
    }

    /// The whole resolved hash, including non-cop sections such as `Language`.
    pub fn raw(&self) -> &Mapping {
        &self.raw
    }

    /// `AllCops/Include` and `AllCops/Exclude`.
    pub fn file_matcher(&self) -> &FileMatcher {
        &self.matcher
    }

    /// The cop's own `Include`/`Exclude`, when it sets either.
    pub fn cop_file_matcher(&self, name: &str) -> Option<&FileMatcher> {
        self.cop_matchers.get(name)
    }

    /// Whether a cop should inspect a file (`relative_path` is relative to
    /// [`LoadedConfig::root`]). Mirrors RuboCop's `Cop::Base#relevant_file?`:
    /// only the cop's own `Include`/`Exclude` (merged with its department,
    /// see [`LoadedConfig::cop_file_matcher`]) is consulted. `AllCops`'s
    /// `Include`/`Exclude` is matched exactly once, at file discovery
    /// (`discover::discover`, which also applies RuboCop's shebang fallback
    /// for extension-less scripts); re-matching it here — without that
    /// fallback — would silently disable every cop for a shebang-only
    /// script that discovery had already selected as a target.
    pub fn is_cop_enabled_for(&self, name: &str, relative_path: &Path) -> bool {
        let Some(cop) = self.cop(name) else { return false };
        if !cop.enabled {
            return false;
        }
        self.cop_matchers.get(name).is_none_or(|matcher| matcher.is_target(relative_path))
    }

    /// Like [`LoadedConfig::is_cop_enabled_for`] but ignoring the cop's
    /// `Enabled` flag: RuboCop's `--only` runs a disabled cop while still
    /// honouring its `Include`/`Exclude`.
    pub fn is_cop_targeting(&self, name: &str, relative_path: &Path) -> bool {
        self.cop_matchers.get(name).is_none_or(|matcher| matcher.is_target(relative_path))
    }

    /// Extension gems requested through `require:`/`plugins:` that could not
    /// be located on disk, so their cops still run with only RuboCop's own
    /// default settings.
    pub fn requested_extensions(&self) -> Vec<&str> {
        self.extensions
            .iter()
            .map(String::as_str)
            .filter(|name| KNOWN_EXTENSIONS.contains(name))
            .filter(|name| !self.resolved_extensions.contains(*name))
            .collect()
    }

    /// Every `require:`/`plugins:` entry, in load order.
    pub fn required_features(&self) -> impl Iterator<Item = &str> {
        self.extensions.iter().map(String::as_str)
    }

    /// Non-fatal configuration warnings (obsolete parameters and renames with
    /// `severity: warning`).
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Writes the same YAML shape as `rubocop --show-cops`.
    ///
    /// `only` filters cop names, supporting `*` wildcards like RuboCop's
    /// `--show-cops Style/*`.
    pub fn render_show_cops(
        &self,
        out: &mut impl Write,
        only: Option<&[&str]>,
    ) -> std::io::Result<()> {
        let show_all = only.is_none_or(<[&str]>::is_empty);
        let selected: Vec<(&str, &CopConfig)> = self
            .cops()
            .filter(|(name, _)| match only {
                None | Some([]) => true,
                Some(patterns) => patterns.iter().any(|pattern| matches_cop(pattern, name)),
            })
            .collect();

        if show_all {
            writeln!(
                out,
                "# Available cops ({}) + config for {}: ",
                self.cops.len(),
                self.root.display()
            )?;
        }

        let mut current_department: Option<&str> = None;
        for (name, cop) in selected {
            let department = name.rsplit_once('/').map_or("", |(dept, _)| dept);
            if show_all && current_department != Some(department) {
                let count = self
                    .cops
                    .keys()
                    .filter(|other| other.starts_with(&format!("{department}/")))
                    .count();
                writeln!(out, "# Department '{department}' ({count}):")?;
                current_department = Some(department);
            }
            writeln!(out, "{name}:")?;
            let mut body = String::new();
            emit_mapping(cop.raw(), 2, &mut body);
            out.write_all(body.as_bytes())?;
            writeln!(out)?;
        }
        Ok(())
    }

    /// A stable hash of everything that affects linting.
    ///
    /// FNV-1a over a canonical rendering, so the value does not depend on the
    /// Rust version the way `DefaultHasher` does.
    pub fn effective_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let feed = |bytes: &[u8], hash: &mut u64| {
            for byte in bytes {
                *hash ^= u64::from(*byte);
                *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        let mut canonical = String::new();
        emit_mapping(self.all_cops.raw(), 0, &mut canonical);
        feed(b"AllCops\n", &mut hash);
        feed(canonical.as_bytes(), &mut hash);
        for (name, cop) in self.cops() {
            feed(name.as_bytes(), &mut hash);
            feed(if cop.enabled { b"\x01" } else { b"\x00" }, &mut hash);
            let mut rendered = String::new();
            emit_mapping(cop.raw(), 0, &mut rendered);
            feed(rendered.as_bytes(), &mut hash);
        }
        for feature in &self.extensions {
            feed(feature.as_bytes(), &mut hash);
        }
        hash
    }
}

/// A cop whose configuration was overridden with `~` is deleted from the
/// merged hash, but RuboCop still runs it with an empty config
/// (`Config#for_cop` returns `{}`), so keep it as an unconfigured cop.
fn backfill_unconfigured_cops(
    raw: &Mapping,
    loaded_path: Option<&Path>,
    disabled_by_default: bool,
    cops: &mut BTreeMap<String, CopConfig>,
) {
    for name in DEFAULT_CONFIG.keys() {
        if !name.contains('/') || raw.contains_key(name) || cops.contains_key(name) {
            continue;
        }
        let mut params = Mapping::new();
        let enabled_value = enabled_value(name, &params, raw, disabled_by_default);
        params.insert("Enabled", enabled_value.clone());
        cops.insert(
            name.to_string(),
            CopConfig {
                enabled: enabled_value.as_bool().unwrap_or(false),
                severity: None,
                include: Vec::new(),
                exclude: Vec::new(),
                options: std::iter::once(("Enabled".to_string(), enabled_value.clone())).collect(),
                source: loaded_path.map(Path::to_path_buf),
                enabled_value,
                raw: params,
            },
        );
    }
}

/// `Config#enable_cop?`, returning the value `Config#for_cop` would report.
fn enabled_value(
    name: &str,
    params: &Mapping,
    raw: &Mapping,
    disabled_by_default: bool,
) -> YamlValue {
    let configured = params.get("Enabled");
    if configured.and_then(YamlValue::as_bool) == Some(true) || name == "Lint/Syntax" {
        return YamlValue::Bool(true);
    }
    let cop_enabled = configured.cloned().unwrap_or(YamlValue::Bool(!disabled_by_default));
    if cop_enabled.as_str() == Some("override_department") {
        return YamlValue::Bool(true);
    }
    if let Some((department, _)) = name.rsplit_once('/') {
        let disabled =
            raw.get_mapping(department).and_then(|d| d.get("Enabled")).and_then(YamlValue::as_bool)
                == Some(false);
        if disabled {
            return YamlValue::Bool(false);
        }
    }
    cop_enabled
}

/// RuboCop's `ShowCops::WildcardMatcher`/`ExactMatcher`.
fn matches_cop(pattern: &str, name: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == name;
    }
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or("");
    if !name.starts_with(first) {
        return false;
    }
    let mut rest = &name[first.len()..];
    let mut last_empty = true;
    for part in parts {
        last_empty = part.is_empty();
        if part.is_empty() {
            continue;
        }
        // `File::FNM_PATHNAME` keeps `*` from crossing a `/`.
        let Some(found) = rest.find(part) else { return false };
        if rest[..found].contains('/') {
            return false;
        }
        rest = &rest[found + part.len()..];
    }
    if last_empty {
        !rest.contains('/')
    } else {
        rest.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcards_do_not_cross_departments() {
        assert!(matches_cop("Style/*", "Style/Alias"));
        assert!(!matches_cop("Style/*", "Layout/Alias"));
        assert!(matches_cop("*/Alias", "Style/Alias"));
        assert!(matches_cop("Style/Ali*", "Style/Alias"));
        assert!(!matches_cop("Style/Ali*", "Style/Blias"));
        assert!(matches_cop("Style/Alias", "Style/Alias"));
        assert!(!matches_cop("Style/Alia", "Style/Alias"));
    }
}
