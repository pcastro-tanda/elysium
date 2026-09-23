//! Finding, reading and resolving `.rubocop.yml` files.
//!
//! Ported from `lib/rubocop/config_loader.rb`,
//! `lib/rubocop/config_loader_resolver.rb`, `lib/rubocop/config_finder.rb` and
//! `lib/rubocop/config.rb`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::defaults::{DEFAULT_CONFIG, DEPARTMENTS};
use crate::error::ConfigError;
use crate::gems::{self, GemSearch};
use crate::merge::{
    inherit_mode_for, merge, override_department_setting_for_cops,
    override_enabled_for_disabled_departments, MergeOpts,
};
use crate::obsoletion::RULES;
use crate::paths;
use crate::resolved::LoadedConfig;
use crate::yaml::{parse_document, Mapping, YamlValue};

/// RuboCop's `ConfigFinder::DOTFILE`.
pub const DOTFILE: &str = ".rubocop.yml";
/// RuboCop's `ConfigFinder::XDG_CONFIG`.
pub const XDG_CONFIG: &str = "config.yml";
/// RuboCop's default target Ruby version (`TargetRuby::DEFAULT_VERSION`).
pub const DEFAULT_RUBY_VERSION: f32 = 2.7;

/// Loads RuboCop configuration files.
#[derive(Debug, Clone)]
pub struct ConfigLoader {
    cwd: PathBuf,
    project_root: Option<PathBuf>,
    home: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
    gem_search: GemSearch,
    allow_gem_lookup: bool,
    ignore_parent_exclusion: bool,
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// State shared across the files of one `load` call.
#[derive(Debug, Default)]
struct LoadState {
    /// Files currently being resolved, for circular-inheritance detection.
    stack: Vec<PathBuf>,
    /// `require:`/`plugins:` entries, in the order they were seen.
    extensions: Vec<String>,
    /// Non-fatal obsoletion messages.
    warnings: Vec<String>,
}

/// One loaded file after its own inheritance was resolved.
#[derive(Debug)]
struct FileConfig {
    hash: Mapping,
    path: PathBuf,
}

impl ConfigLoader {
    /// A loader rooted at the process working directory, allowed to look for
    /// gems in the environment.
    pub fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            project_root: None,
            home: std::env::var_os("HOME").map(PathBuf::from),
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            gem_search: GemSearch { extra_roots: Vec::new(), use_environment: true },
            allow_gem_lookup: true,
            ignore_parent_exclusion: false,
        }
    }

    /// Overrides the working directory, which is where paths in config files
    /// whose name does not start with `.rubocop` are anchored.
    #[must_use]
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = cwd.into();
        self
    }

    /// Overrides the inferred project root (RuboCop infers it from the
    /// outermost `Gemfile`/`gems.rb`).
    #[must_use]
    pub fn with_project_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.project_root = Some(root.into());
        self
    }

    /// Overrides the home directory used for `~/.rubocop.yml`.
    #[must_use]
    pub fn with_home(mut self, home: impl Into<PathBuf>) -> Self {
        self.home = Some(home.into());
        self
    }

    /// Overrides `$XDG_CONFIG_HOME`.
    #[must_use]
    pub fn with_xdg_config_home(mut self, dir: impl Into<PathBuf>) -> Self {
        self.xdg_config_home = Some(dir.into());
        self
    }

    /// Enables or disables `inherit_gem` resolution.
    #[must_use]
    pub fn with_gem_lookup(mut self, allow: bool) -> Self {
        self.allow_gem_lookup = allow;
        self
    }

    /// Restricts gem lookup to the given roots (each may be a gem home, a
    /// `gems` directory, or a Bundler path) and stops consulting the
    /// environment.
    #[must_use]
    pub fn with_gem_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.gem_search = GemSearch { extra_roots: roots, use_environment: false };
        self
    }

    /// RuboCop's `--ignore-parent-exclusion`.
    #[must_use]
    pub fn ignore_parent_exclusion(mut self, ignore: bool) -> Self {
        self.ignore_parent_exclusion = ignore;
        self
    }

    /// The project root: the directory of the outermost `Gemfile`/`gems.rb`
    /// at or above the working directory, if any.
    pub fn project_root(&self) -> Option<PathBuf> {
        if let Some(root) = &self.project_root {
            return Some(root.clone());
        }
        let mut found = None;
        for dir in paths::ancestors(&self.cwd, None) {
            for name in ["Gemfile", "gems.rb"] {
                if dir.join(name).is_file() {
                    found = Some(dir.to_path_buf());
                }
            }
        }
        found
    }

    /// RuboCop's `ConfigLoader.configuration_file_for`: the nearest
    /// `.rubocop.yml` at or above `dir` (never past the project root), then the
    /// project's `.config/` copies, then `~/.rubocop.yml`, then the XDG config.
    /// `None` means "use the bundled defaults only".
    pub fn find_config_file(&self, dir: &Path) -> Option<PathBuf> {
        let dir = paths::expand(dir, &self.cwd);
        let project_root = self.project_root();
        if let Some(found) = find_file_upwards(DOTFILE, &dir, project_root.as_deref()) {
            return Some(found);
        }
        if let Some(root) = &project_root {
            let dotfile = root.join(".config").join(DOTFILE);
            if dotfile.is_file() {
                return Some(dotfile);
            }
            let xdg = root.join(".config").join("rubocop").join(XDG_CONFIG);
            if xdg.is_file() {
                return Some(xdg);
            }
        }
        if let Some(home) = &self.home {
            let dotfile = home.join(DOTFILE);
            if dotfile.is_file() {
                return Some(dotfile);
            }
        }
        let xdg_home = self
            .xdg_config_home
            .clone()
            .or_else(|| self.home.as_ref().map(|h| h.join(".config")))?;
        let xdg = paths::expand(&xdg_home, &self.cwd).join("rubocop").join(XDG_CONFIG);
        xdg.is_file().then_some(xdg)
    }

    /// Loads a configuration file (or, with `None`, only the bundled
    /// defaults) and resolves it against them.
    pub fn load(&self, path: Option<&Path>) -> Result<LoadedConfig, ConfigError> {
        let mut state = LoadState::default();
        let (hash, loaded_path) = match path {
            None => (Mapping::new(), None),
            Some(path) => {
                let path = paths::expand(path, &self.cwd);
                let mut file = self.load_file(&path, &mut state)?;
                if !self.ignore_parent_exclusion {
                    self.add_excludes_from_files(&mut file, &mut state)?;
                }
                (file.hash, Some(file.path))
            }
        };
        let gem_search_start = self.project_root().unwrap_or_else(|| {
            loaded_path.as_deref().and_then(Path::parent).unwrap_or(&self.cwd).to_path_buf()
        });
        let (resolved, resolved_extensions) =
            self.merge_with_default(hash, &gem_search_start, &state.extensions)?;
        let root = match &loaded_path {
            Some(path) => base_dir_for_path_parameters(path, &self.cwd, self.home.as_deref()),
            None => self.cwd.clone(),
        };
        Ok(LoadedConfig::new(
            resolved,
            loaded_path,
            root,
            state.extensions,
            resolved_extensions,
            state.warnings,
        ))
    }

    /// RuboCop's `ConfigLoader.load_file`: read, resolve `inherit_gem`,
    /// `inherit_from`, qualify cop names, reject obsolete settings and make
    /// `Exclude` paths absolute.
    fn load_file(&self, path: &Path, state: &mut LoadState) -> Result<FileConfig, ConfigError> {
        if state.stack.iter().any(|p| p == path) {
            return Err(ConfigError::CircularInheritance(path.to_path_buf()));
        }
        state.stack.push(path.to_path_buf());
        let result = self.load_file_inner(path, state);
        state.stack.pop();
        result
    }

    fn load_file_inner(
        &self,
        path: &Path,
        state: &mut LoadState,
    ) -> Result<FileConfig, ConfigError> {
        let mut hash = read_yaml_configuration(path)?;

        for key in ["plugins", "require"] {
            if let Some(value) = hash.remove(key) {
                for entry in value.to_string_list() {
                    if !state.extensions.contains(&entry) {
                        state.extensions.push(entry);
                    }
                }
            }
        }

        let mut inherit_from: Vec<String> =
            hash.get("inherit_from").map(YamlValue::to_string_list).unwrap_or_default();
        if let Some(YamlValue::Mapping(gems)) = hash.remove("inherit_gem") {
            for (gem, value) in gems.iter() {
                for relative in value.to_string_list().iter().rev() {
                    let resolved = self.gem_config_path(path, gem, relative)?;
                    inherit_from.insert(0, resolved.to_string_lossy().into_owned());
                }
            }
        }
        hash.remove("inherit_from");

        self.resolve_inheritance(path, &mut hash, &inherit_from, state)?;
        add_missing_namespaces(&mut hash)?;

        let target_ruby = target_ruby_version(&hash, path, &self.cwd, self.home.as_deref());
        let extensions: HashSet<String> = state.extensions.iter().cloned().collect();
        let obsoletions = RULES.check(&hash, path, target_ruby, &extensions);
        if !obsoletions.errors.is_empty() {
            return Err(ConfigError::ObsoleteCop(obsoletions.errors.join("\n")));
        }
        state.warnings.extend(obsoletions.warnings);

        let base_dir = base_dir_for_path_parameters(path, &self.cwd, self.home.as_deref());
        make_excludes_absolute(&mut hash, &base_dir);

        Ok(FileConfig { hash, path: path.to_path_buf() })
    }

    /// `ConfigLoaderResolver#resolve_inheritance`.
    fn resolve_inheritance(
        &self,
        path: &Path,
        hash: &mut Mapping,
        inherit_from: &[String],
        state: &mut LoadState,
    ) -> Result<(), ConfigError> {
        let mut bases: Vec<(String, FileConfig)> = Vec::new();
        for entry in inherit_from {
            if entry.starts_with("http://") || entry.starts_with("https://") {
                return Err(ConfigError::RemoteUnsupported {
                    path: path.to_path_buf(),
                    url: entry.clone(),
                });
            }
            let config_dir = path.parent().unwrap_or(&self.cwd);
            if paths::is_glob(entry) {
                for expanded in paths::glob(&paths::expand(Path::new(entry), &self.cwd)) {
                    bases.push((entry.clone(), self.load_file(&expanded, state)?));
                }
            } else {
                let resolved = paths::expand(Path::new(entry), config_dir);
                bases.push((entry.clone(), self.load_file(&resolved, state)?));
            }
        }

        for (_, base) in bases.iter().rev() {
            override_department_setting_for_cops(&base.hash, hash);
            override_enabled_for_disabled_departments(&base.hash, hash);

            for (key, value) in base.hash.iter() {
                let YamlValue::Mapping(base_cop) = value else { continue };
                let merged = match hash.get(key) {
                    Some(YamlValue::Mapping(derived_cop)) => merge(
                        base_cop,
                        derived_cop,
                        MergeOpts { inherit_mode: inherit_mode_for(hash, key), unset_nil: false },
                    ),
                    Some(_) => continue,
                    None => base_cop.clone(),
                };
                let has_include = merged.contains_key("Include");
                hash.insert(key, YamlValue::Mapping(merged));
                if has_include {
                    fix_include_paths(&base.path, hash, path, key);
                }
            }
        }
        Ok(())
    }

    fn gem_config_path(
        &self,
        config_path: &Path,
        gem: &str,
        relative: &str,
    ) -> Result<PathBuf, ConfigError> {
        if gem == "rubocop" {
            return Err(ConfigError::InheritFromRubocopGem);
        }
        if !self.allow_gem_lookup {
            return Err(ConfigError::GemNotFound {
                gem: gem.to_string(),
                version: None,
                searched: Vec::new(),
            });
        }
        let start = self
            .project_root()
            .unwrap_or_else(|| config_path.parent().unwrap_or(&self.cwd).to_path_buf());
        self.gem_search.config_path(&start, gem, relative)
    }

    /// `ConfigLoader.add_excludes_from_files`: `AllCops/Exclude` from the
    /// outermost `.rubocop.yml` of the project also applies here.
    fn add_excludes_from_files(
        &self,
        config: &mut FileConfig,
        state: &mut LoadState,
    ) -> Result<(), ConfigError> {
        let start = config.path.parent().unwrap_or(&self.cwd).to_path_buf();
        let root = self.project_root();
        let Some(highest) = find_last_file_upwards(DOTFILE, &start, root.as_deref()) else {
            return Ok(());
        };
        if highest == config.path {
            return Ok(());
        }
        let mut nested = LoadState { stack: Vec::new(), ..LoadState::default() };
        let highest = self.load_file(&highest, &mut nested)?;
        state.warnings.extend(nested.warnings);
        let Some(extra) = highest.hash.get_mapping("AllCops").map(|a| a.get_string_list("Exclude"))
        else {
            return Ok(());
        };
        if extra.is_empty() {
            return Ok(());
        }
        if config.hash.get_mapping("AllCops").is_none() {
            config.hash.insert("AllCops", YamlValue::Mapping(Mapping::new()));
        }
        let all_cops = config.hash.get_mapping_mut("AllCops").expect("AllCops is a mapping");
        let mut excludes = all_cops.get_string_list("Exclude");
        for path in extra {
            if !excludes.contains(&path) {
                excludes.push(path);
            }
        }
        all_cops.insert(
            "Exclude",
            YamlValue::Array(excludes.into_iter().map(YamlValue::String).collect()),
        );
        Ok(())
    }

    /// `ConfigLoaderResolver#merge_with_default`, extended to inject the
    /// gem-shipped defaults of every resolvable `require:`/`plugins:` entry
    /// below `config/default.yml`, the way RuboCop's
    /// `Plugin::ConfigurationIntegrator` and `ConfigLoader.inject_defaults!`
    /// do for plugins and legacy `require:` extensions respectively. Returns
    /// the subset of `extensions` that were found on disk (whether or not
    /// they ship a `config/default.yml`), so the caller can stop warning
    /// about them.
    fn merge_with_default(
        &self,
        user: Mapping,
        gem_search_start: &Path,
        extensions: &[String],
    ) -> Result<(Mapping, Vec<String>), ConfigError> {
        let mut default = DEFAULT_CONFIG.clone();
        // RuboCop loads `config/default.yml` through `load_file` as well, and
        // that file is not named `.rubocop*`, so its `Exclude` entries are made
        // absolute against the working directory.
        make_excludes_absolute(&mut default, &self.cwd);

        let mut resolved_extensions = Vec::new();
        if self.allow_gem_lookup {
            for gem in extensions {
                if gem == "rubocop" {
                    continue;
                }
                match self.gem_search.extension_defaults(gem_search_start, gem) {
                    Ok(gems::ExtensionDefaults::Found(path)) => {
                        let mut extension_default = read_yaml_configuration(&path)?;
                        make_excludes_absolute(&mut extension_default, &self.cwd);
                        default = merge_extension_defaults(&default, &extension_default);
                        resolved_extensions.push(gem.clone());
                    }
                    Ok(gems::ExtensionDefaults::NotShipped) => {
                        resolved_extensions.push(gem.clone());
                    }
                    Err(_) => {
                        // Not found on disk; the caller keeps warning about it.
                    }
                }
            }
        }

        let all_cops = user.get_mapping("AllCops");
        let disabled_by_default =
            all_cops.and_then(|a| a.get("DisabledByDefault")).is_some_and(YamlValue::is_truthy);
        let enabled_by_default =
            all_cops.and_then(|a| a.get("EnabledByDefault")).is_some_and(YamlValue::is_truthy);

        if disabled_by_default || enabled_by_default {
            let keys: Vec<String> = default.keys().map(str::to_string).collect();
            for key in keys {
                if let Some(params) = default.get_mapping_mut(&key) {
                    params.insert("Enabled", YamlValue::Bool(!disabled_by_default));
                }
            }
        }

        let mut user = user;
        if disabled_by_default {
            handle_disabled_by_default(&mut user, &mut default);
        }
        override_enabled_for_disabled_departments(&default, &mut user);

        let inherit_mode = user.get_mapping("inherit_mode").cloned();
        let merged = merge(
            &default,
            &user,
            MergeOpts { inherit_mode: inherit_mode.as_ref(), unset_nil: true },
        );
        Ok((merged, resolved_extensions))
    }
}

/// `Plugin::ConfigurationIntegrator#merge_plugin_config_into_default_config!`
/// combined with `#merge_all_cop_settings`, simplified to the one special
/// case elysium's supported extensions need: RuboCop always unions an
/// extension's `AllCops/Exclude` into the accumulated defaults rather than
/// letting it replace them (excluding `bin/*` should not un-exclude
/// `**/app/assets/**/*`); every other key the extension sets overrides
/// RuboCop's own default, exactly like a plain recursive hash merge.
fn merge_extension_defaults(base: &Mapping, extension: &Mapping) -> Mapping {
    let mut merged = merge(base, extension, MergeOpts { inherit_mode: None, unset_nil: false });
    let Some(extension_all_cops) = extension.get_mapping("AllCops") else { return merged };
    if !extension_all_cops.contains_key("Exclude") {
        return merged;
    }
    let mut excludes =
        base.get_mapping("AllCops").map(|a| a.get_string_list("Exclude")).unwrap_or_default();
    for path in extension_all_cops.get_string_list("Exclude") {
        if !excludes.contains(&path) {
            excludes.push(path);
        }
    }
    let all_cops = merged.get_mapping_mut("AllCops").expect("extension set AllCops");
    all_cops
        .insert("Exclude", YamlValue::Array(excludes.into_iter().map(YamlValue::String).collect()));
    merged
}

/// `ConfigLoaderResolver#handle_disabled_by_default`.
fn handle_disabled_by_default(user: &mut Mapping, default: &mut Mapping) {
    let enabled_departments: Vec<String> = user
        .iter()
        .filter(|(key, _)| !key.contains('/'))
        .filter(|(_, value)| {
            value.as_mapping().and_then(|m| m.get("Enabled")).is_some_and(YamlValue::is_truthy)
        })
        .map(|(key, _)| key.to_string())
        .collect();
    for department in enabled_departments {
        let prefix = format!("{department}/");
        let cops: Vec<String> =
            default.keys().filter(|k| k.starts_with(&prefix)).map(str::to_string).collect();
        for cop in cops {
            let original = DEFAULT_CONFIG
                .get_mapping(&cop)
                .and_then(|c| c.get("Enabled"))
                .cloned()
                .unwrap_or(YamlValue::Bool(true));
            if let Some(params) = default.get_mapping_mut(&cop) {
                params.insert("Enabled", original);
            }
        }
    }

    let keys: Vec<String> = user.keys().map(str::to_string).collect();
    for key in keys {
        let Some(YamlValue::Mapping(params)) = user.get(&key) else { continue };
        let mut with_enabled = Mapping::new();
        with_enabled.insert("Enabled", YamlValue::Bool(true));
        let merged = with_enabled.merged_with(params);
        user.insert(key, YamlValue::Mapping(merged));
    }
}

/// `ConfigLoader.load_yaml_configuration`, minus ERB.
fn read_yaml_configuration(path: &Path) -> Result<Mapping, ConfigError> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Err(ConfigError::NotFound(path.to_path_buf()))
        }
        Err(source) => return Err(ConfigError::Io { path: path.to_path_buf(), source }),
    };
    if contents.contains("<%") {
        return Err(ConfigError::ErbUnsupported(path.to_path_buf()));
    }
    match parse_document(&contents) {
        Ok(None) => Ok(Mapping::new()),
        Ok(Some(YamlValue::Mapping(hash))) => Ok(hash),
        Ok(Some(_)) => Err(ConfigError::Malformed(path.to_path_buf())),
        Err(message) => Err(ConfigError::Yaml { path: path.to_path_buf(), message }),
    }
}

/// `ConfigLoader.add_missing_namespaces` plus
/// `Cop::Registry.qualified_cop_name`.
fn add_missing_namespaces(hash: &mut Mapping) -> Result<(), ConfigError> {
    let keys: Vec<String> = hash.keys().map(str::to_string).collect();
    for key in keys {
        if RULES.is_deprecated_name(&key) {
            continue;
        }
        let qualified = qualified_cop_name(&key)?;
        if qualified == key {
            continue;
        }
        if let Some(value) = hash.remove(&key) {
            hash.insert(qualified, value);
        }
    }
    Ok(())
}

/// Qualifies a bare or misplaced cop name against the default configuration.
fn qualified_cop_name(name: &str) -> Result<String, ConfigError> {
    if DEFAULT_CONFIG.contains_key(name) || name == "AllCops" || name == "inherit_mode" {
        return Ok(name.to_string());
    }
    let bare = name.rsplit_once('/').map_or(name, |(_, cop)| cop);
    let candidates: Vec<String> = DEPARTMENTS
        .iter()
        .map(|department| format!("{department}/{bare}"))
        .filter(|candidate| DEFAULT_CONFIG.contains_key(candidate))
        .collect();
    match candidates.len() {
        0 => Ok(name.to_string()),
        1 => Ok(candidates.into_iter().next().expect("one candidate")),
        _ => Err(ConfigError::AmbiguousCopName { name: name.to_string(), candidates }),
    }
}

/// `Config#make_excludes_absolute`.
fn make_excludes_absolute(hash: &mut Mapping, base_dir: &Path) {
    let keys: Vec<String> = hash.keys().map(str::to_string).collect();
    for key in keys {
        let Some(params) = hash.get_mapping_mut(&key) else { continue };
        let Some(YamlValue::Array(excludes)) = params.get("Exclude") else { continue };
        let absolute: Vec<YamlValue> = excludes
            .iter()
            .map(|value| match value {
                YamlValue::String(text) if !Path::new(text).is_absolute() => YamlValue::String(
                    paths::expand(Path::new(text), base_dir).to_string_lossy().into_owned(),
                ),
                other => other.clone(),
            })
            .collect();
        params.insert("Exclude", YamlValue::Array(absolute));
    }
}

/// `ConfigLoaderResolver#fix_include_paths`: `Include` patterns of an
/// inherited `.rubocop*` file are relative to that file's directory.
fn fix_include_paths(base_config_path: &Path, hash: &mut Mapping, path: &Path, key: &str) {
    let is_rubocop_file = base_config_path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(".rubocop"));
    if !is_rubocop_file {
        return;
    }
    let base_dir = base_config_path.parent().unwrap_or(Path::new("."));
    let derived_dir = path.parent().unwrap_or(Path::new("."));
    let Some(params) = hash.get_mapping_mut(key) else { return };
    let includes = params.get_string_list("Include");
    let fixed: Vec<YamlValue> = includes
        .iter()
        .map(|include| {
            let joined = paths::expand(Path::new(include), base_dir);
            YamlValue::String(paths::relative(&joined, derived_dir).to_string_lossy().into_owned())
        })
        .collect();
    params.insert("Include", YamlValue::Array(fixed));
}

/// `Config#base_dir_for_path_parameters`.
fn base_dir_for_path_parameters(loaded_path: &Path, cwd: &Path, home: Option<&Path>) -> PathBuf {
    let is_dotfile =
        loaded_path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(".rubocop"));
    let is_home_dotfile = home.is_some_and(|home| loaded_path == home.join(DOTFILE));
    if is_dotfile && !is_home_dotfile {
        loaded_path.parent().map_or_else(|| cwd.to_path_buf(), Path::to_path_buf)
    } else {
        cwd.to_path_buf()
    }
}

/// `TargetRuby`, minus the gemspec source (which needs a Ruby parse).
fn target_ruby_version(hash: &Mapping, path: &Path, cwd: &Path, home: Option<&Path>) -> f32 {
    if let Some(version) = hash
        .get_mapping("AllCops")
        .and_then(|a| a.get("TargetRubyVersion"))
        .and_then(YamlValue::as_f32)
    {
        return version;
    }
    let base_dir = base_dir_for_path_parameters(path, cwd, home);
    if let Some(file) = find_file_upwards(".ruby-version", &base_dir, None) {
        if let Some(version) = std::fs::read_to_string(file)
            .ok()
            .and_then(|text| major_minor(text.trim().trim_start_matches("ruby-")))
        {
            return version;
        }
    }
    if let Some(file) = find_file_upwards(".tool-versions", &base_dir, None) {
        if let Some(version) = std::fs::read_to_string(file).ok().and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("ruby "))
                .and_then(|rest| major_minor(rest.trim()))
        }) {
            return version;
        }
    }
    if let Some(lockfile) = gems::find_lockfile(&base_dir) {
        if let Some(version) = std::fs::read_to_string(lockfile).ok().and_then(|text| {
            let mut in_section = false;
            for line in text.lines() {
                if line.trim() == "RUBY VERSION" {
                    in_section = true;
                } else if in_section {
                    return line.trim().strip_prefix("ruby ").and_then(major_minor);
                }
            }
            None
        }) {
            return version;
        }
    }
    DEFAULT_RUBY_VERSION
}

/// `"3.4.2p12"` becomes `3.4`.
fn major_minor(text: &str) -> Option<f32> {
    let mut parts = text.split('.');
    let major: u32 = parts.next()?.trim().parse().ok()?;
    let minor: String = parts.next()?.chars().take_while(char::is_ascii_digit).collect();
    format!("{major}.{minor}").parse().ok()
}

/// `FileFinder#find_file_upwards`.
fn find_file_upwards(name: &str, start: &Path, stop: Option<&Path>) -> Option<PathBuf> {
    for dir in paths::ancestors(start, stop) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// `FileFinder#find_last_file_upwards`.
fn find_last_file_upwards(name: &str, start: &Path, stop: Option<&Path>) -> Option<PathBuf> {
    let mut last = None;
    for dir in paths::ancestors(start, stop) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            last = Some(candidate);
        }
    }
    last
}
