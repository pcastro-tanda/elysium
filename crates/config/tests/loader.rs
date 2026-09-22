//! Behaviour tests mirroring `spec/rubocop/config_loader_spec.rb` (RuboCop
//! 1.82.1). Line references in comments point at that spec.

use std::path::{Path, PathBuf};

use config::{ConfigError, ConfigLoader, LoadedConfig, NewCops};
use tempfile::TempDir;

/// A project directory with a loader anchored in it.
struct Project {
    dir: TempDir,
}

impl Project {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("home")).expect("home");
        Self { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.dir.path().join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(&path, contents).expect("write");
        path
    }

    fn loader(&self) -> ConfigLoader {
        ConfigLoader::new()
            .with_cwd(self.dir.path())
            .with_project_root(self.dir.path())
            .with_home(self.dir.path().join("home"))
            .with_xdg_config_home(self.dir.path().join("home/.config"))
            .with_gem_roots(Vec::new())
    }

    fn load(&self, relative: &str) -> LoadedConfig {
        self.loader().load(Some(Path::new(relative))).expect("config loads")
    }

    fn abs(&self, relative: &str) -> String {
        self.dir.path().join(relative).to_string_lossy().into_owned()
    }
}

fn enabled(config: &LoadedConfig, cop: &str) -> bool {
    config.cop(cop).unwrap_or_else(|| panic!("{cop} is unknown")).enabled
}

#[test]
fn defaults_only_matches_rubocop_defaults() {
    let project = Project::new();
    let config = project.loader().load(None).expect("defaults load");
    assert_eq!(config.all_cops().new_cops, NewCops::Pending);
    assert!(enabled(&config, "Layout/TrailingWhitespace"));
    // Pending cops stay off until `NewCops: enable`.
    let pending = config
        .cops()
        .find(|(_, cop)| cop.is_pending())
        .map(|(name, _)| name.to_string())
        .expect("default.yml has pending cops");
    assert!(!enabled(&config, &pending), "{pending} should stay disabled");
    assert!(config.cop("Style/NotACop").is_none());
    assert_eq!(config.root(), project.path());
}

#[test]
fn inheritance_chain_merges_parent_then_child() {
    // spec/rubocop/config_loader_spec.rb:1057 "inherits from a parent and grandparent file"
    let project = Project::new();
    project.write(
        "grandparent.yml",
        "Style/Alias:\n  Enabled: false\nMetrics/MethodLength:\n  Max: 5\n",
    );
    project.write(
        "parent.yml",
        "inherit_from: grandparent.yml\nMetrics/MethodLength:\n  Max: 7\nStyle/AndOr:\n  Enabled: false\n",
    );
    project.write(".rubocop.yml", "inherit_from: parent.yml\nStyle/AndOr:\n  Enabled: true\n");

    let config = project.load(".rubocop.yml");
    assert!(!enabled(&config, "Style/Alias"), "grandparent disables Style/Alias");
    assert!(enabled(&config, "Style/AndOr"), "child re-enables Style/AndOr");
    assert_eq!(
        config.cop("Metrics/MethodLength").unwrap().options.get("Max"),
        Some(&config::YamlValue::Int(7))
    );
}

#[test]
fn todo_file_is_just_another_inherited_file() {
    let project = Project::new();
    project.write(".rubocop_todo.yml", "Style/Documentation:\n  Exclude:\n    - 'app/legacy.rb'\n");
    project.write(".rubocop.yml", "inherit_from:\n  - .rubocop_todo.yml\n");

    let config = project.load(".rubocop.yml");
    assert_eq!(
        config.cop("Style/Documentation").unwrap().exclude,
        [project.abs("app/legacy.rb")],
        "Exclude from the todo file is absolute relative to that file"
    );
    assert!(!config.is_cop_enabled_for("Style/Documentation", Path::new("app/legacy.rb")));
    assert!(config.is_cop_enabled_for("Style/Documentation", Path::new("app/fresh.rb")));
}

#[test]
fn inherit_mode_merge_unions_and_override_replaces() {
    // spec/rubocop/config_loader_spec.rb:595 and :661
    let project = Project::new();
    project.write(
        ".rubocop_parent.yml",
        concat!(
            "Style/For:\n  Exclude:\n    - 'spec/models/expense_spec.rb'\n",
            "    - 'spec/models/group_spec.rb'\n",
            "Style/Dir:\n  Exclude:\n    - 'spec/models/expense_spec.rb'\n",
        ),
    );
    project.write(
        ".rubocop.yml",
        concat!(
            "inherit_from: .rubocop_parent.yml\n",
            "inherit_mode:\n  merge:\n    - Exclude\n",
            "AllCops:\n  Exclude:\n    - spec/requests/expense_spec.rb\n",
            "Style/For:\n  Exclude:\n    - spec/requests/group_invite_spec.rb\n",
            "Style/Dir:\n  inherit_mode:\n    override:\n      - Exclude\n",
            "  Exclude:\n    - spec/requests/group_invite_spec.rb\n",
        ),
    );

    let config = project.load(".rubocop.yml");
    let mut for_excludes = config.cop("Style/For").unwrap().exclude.clone();
    for_excludes.sort();
    assert_eq!(
        for_excludes,
        [
            project.abs("spec/models/expense_spec.rb"),
            project.abs("spec/models/group_spec.rb"),
            project.abs("spec/requests/group_invite_spec.rb"),
        ]
    );
    assert_eq!(
        config.cop("Style/Dir").unwrap().exclude,
        [project.abs("spec/requests/group_invite_spec.rb")],
        "a per-cop override beats the global merge"
    );
    // AllCops/Exclude is unioned with the bundled defaults (spec line 642).
    let all_excludes = &config.all_cops().exclude;
    assert!(all_excludes.contains(&project.abs("spec/requests/expense_spec.rb")));
    assert!(all_excludes.iter().any(|e| e.ends_with("node_modules/**/*")));
}

#[test]
fn inherited_include_paths_are_rewritten_for_the_deriving_file() {
    // ConfigLoaderResolver#fix_include_paths
    let project = Project::new();
    project.write(
        "nested/.rubocop_shared.yml",
        "Style/Documentation:\n  Include:\n    - 'lib/**/*.rb'\n",
    );
    project.write(".rubocop.yml", "inherit_from: nested/.rubocop_shared.yml\n");

    let config = project.load(".rubocop.yml");
    assert_eq!(config.cop("Style/Documentation").unwrap().include, ["nested/lib/**/*.rb"]);
    assert!(config.is_cop_enabled_for("Style/Documentation", Path::new("nested/lib/a.rb")));
    assert!(!config.is_cop_enabled_for("Style/Documentation", Path::new("lib/a.rb")));
}

#[test]
fn per_cop_exclude_is_absolute_relative_to_the_declaring_file() {
    // spec/rubocop/config_loader_spec.rb:2125 "configuration for CharacterLiteral"
    let project = Project::new();
    project.write(
        "test/.rubocop_rules.yml",
        "Style/CharacterLiteral:\n  Exclude:\n    - blargh/blah.rb\n",
    );
    project.write("test/.rubocop.yml", "inherit_from: .rubocop_rules.yml\n");

    let config = project.load("test/.rubocop.yml");
    assert_eq!(
        config.cop("Style/CharacterLiteral").unwrap().exclude,
        [project.abs("test/blargh/blah.rb")]
    );
    assert_eq!(config.root(), project.path().join("test"));
}

#[test]
fn disabled_by_default_only_enables_mentioned_cops() {
    // spec/rubocop/config_loader_spec.rb:1587
    let project = Project::new();
    project.write(
        ".rubocop.yml",
        "AllCops:\n  DisabledByDefault: true\nStyle/Copyright:\n  Exclude:\n  - foo\n",
    );
    let config = project.load(".rubocop.yml");
    assert!(enabled(&config, "Style/Copyright"), "mentioned cops are enabled");
    assert!(!enabled(&config, "Layout/TrailingWhitespace"));

    // "and a department is enabled" (spec line 1609)
    project.write(".rubocop.yml", "AllCops:\n  DisabledByDefault: true\nStyle:\n  Enabled: true\n");
    let config = project.load(".rubocop.yml");
    assert!(enabled(&config, "Style/Alias"));
    assert!(!enabled(&config, "Layout/HashAlignment"));
    assert!(
        !enabled(&config, "Style/AutoResourceCleanup"),
        "cops disabled in default.yml stay disabled"
    );
}

#[test]
fn enabled_by_default_enables_everything_but_explicit_opt_outs() {
    // spec/rubocop/config_loader_spec.rb:1636
    let project = Project::new();
    project.write(
        ".rubocop.yml",
        "AllCops:\n  EnabledByDefault: true\nLayout/TrailingWhitespace:\n  Enabled: false\n",
    );
    let config = project.load(".rubocop.yml");
    assert!(enabled(&config, "Layout/FirstMethodArgumentLineBreak"));
    assert!(!enabled(&config, "Layout/TrailingWhitespace"));
}

#[test]
fn pending_cops_follow_new_cops() {
    // spec/rubocop/config_loader_spec.rb:1658 "when a new cop is introduced"
    let project = Project::new();
    let pending = {
        let config = project.loader().load(None).expect("defaults");
        let name = config
            .cops()
            .find(|(_, cop)| cop.is_pending())
            .map(|(name, _)| name.to_string())
            .expect("a pending cop exists");
        name
    };

    project.write(".rubocop.yml", "AllCops:\n  NewCops: enable\n");
    assert!(enabled(&project.load(".rubocop.yml"), &pending));

    project.write(".rubocop.yml", "AllCops:\n  NewCops: disable\n");
    assert!(!enabled(&project.load(".rubocop.yml"), &pending));

    project.write(".rubocop.yml", &format!("{pending}:\n  Enabled: true\n"));
    let config = project.load(".rubocop.yml");
    assert!(enabled(&config, &pending));
    assert!(!config.cop(&pending).unwrap().is_pending());

    project.write(".rubocop.yml", "AllCops:\n  DisabledByDefault: true\n");
    assert!(!enabled(&project.load(".rubocop.yml"), &pending));

    project.write(".rubocop.yml", "AllCops:\n  EnabledByDefault: true\n");
    assert!(enabled(&project.load(".rubocop.yml"), &pending));
}

#[test]
fn department_switches_resolve_across_the_inheritance_chain() {
    // spec/rubocop/config_loader_spec.rb:868 "when a department is disabled"
    let project = Project::new();
    project.write(
        "grandparent_rubocop.yml",
        concat!(
            "Layout:\n  Enabled: false\n",
            "Layout/EndOfLine:\n  Enabled: true\n",
            "Naming/FileName:\n  Enabled: pending\n",
            "Metrics/AbcSize:\n  Enabled: true\n",
            "Metrics/PerceivedComplexity:\n  Enabled: true\n",
            "Lint:\n  Enabled: false\n",
        ),
    );
    project.write(
        "parent_rubocop.yml",
        concat!(
            "inherit_from: grandparent_rubocop.yml\n",
            "Metrics:\n  Enabled: false\n",
            "Metrics/AbcSize:\n  Enabled: false\n",
            "Naming:\n  Enabled: false\n",
        ),
    );
    project.write(
        ".rubocop.yml",
        concat!(
            "inherit_from: parent_rubocop.yml\n",
            "Layout:\n  Enabled: false\n",
            "Layout/LineLength:\n  Enabled: true\n",
            "Style:\n  Enabled: false\n",
            "Metrics/MethodLength:\n  Enabled: true\n",
            "Metrics/ClassLength:\n  Enabled: false\n",
            "Lint/RaiseException:\n  Enabled: true\n",
            "Style/AndOr:\n  Enabled: true\n",
        ),
    );

    let config = project.load(".rubocop.yml");
    // Department disabled in grandparent config.
    assert!(!enabled(&config, "Layout/DotPosition"));
    // Enabled in grandparent, department disabled in the user config.
    assert!(!enabled(&config, "Layout/EndOfLine"));
    // Department disabled, cop enabled in the user config.
    assert!(enabled(&config, "Layout/LineLength"));
    assert!(enabled(&config, "Metrics/MethodLength"));
    assert!(!enabled(&config, "Metrics/ClassLength"));
    assert!(!enabled(&config, "Metrics/AbcSize"));
    assert!(!enabled(&config, "Metrics/PerceivedComplexity"));
    // Pending in grandparent, department disabled in parent.
    assert!(!enabled(&config, "Naming/FileName"));
    assert!(!enabled(&config, "Style/Alias"));
    assert!(enabled(&config, "Style/AndOr"));
    assert!(enabled(&config, "Lint/RaiseException"));
    assert!(!enabled(&config, "Lint/StructNewOverride"));
    // Untouched department.
    assert!(enabled(&config, "Bundler/DuplicatedGem"));
}

#[test]
fn department_disabled_in_a_subdirectory_does_not_leak_upwards() {
    // spec/rubocop/config_loader_spec.rb:832
    let project = Project::new();
    project.write("Gemfile", "");
    project.write(".rubocop.yml", "Layout:\n  Enabled: true\n");
    project.write("subdir/.rubocop.yml", "Layout:\n  Enabled: false\n");

    let top = project.load(".rubocop.yml");
    let sub = project.load("subdir/.rubocop.yml");
    assert!(!enabled(&sub, "Layout/LineLength"));
    assert!(enabled(&top, "Layout/LineLength"));
}

#[test]
fn bare_cop_names_are_qualified_and_wrong_departments_fixed() {
    // spec/rubocop/config_loader_spec.rb:1175 "overrides with non-namespaced cops"
    let project = Project::new();
    project.write(".rubocop.yml", "LineLength:\n  Max: 99\nLint/EndOfLine:\n  Enabled: false\n");
    let config = project.load(".rubocop.yml");
    assert_eq!(
        config.cop("Layout/LineLength").unwrap().options.get("Max"),
        Some(&config::YamlValue::Int(99))
    );
    assert!(!enabled(&config, "Layout/EndOfLine"), "wrong department is corrected");
}

#[test]
fn obsolete_cop_names_fail_the_load() {
    // spec/rubocop/config_loader_spec.rb:2025
    let project = Project::new();
    project.write(".rubocop.yml", "Style/MethodMissing:\n  Enabled: true\n");
    let err = project.loader().load(Some(Path::new(".rubocop.yml"))).unwrap_err();
    match err {
        ConfigError::ObsoleteCop(message) => {
            assert!(message.contains("`Style/MethodMissing` cop has been split"), "{message}");
        }
        other => panic!("expected ObsoleteCop, got {other}"),
    }
}

#[test]
fn obsolete_parameters_with_warning_severity_only_warn() {
    let project = Project::new();
    project.write(".rubocop.yml", "Metrics/MethodLength:\n  ExcludedMethods:\n    - foo\n");
    let config = project.load(".rubocop.yml");
    assert_eq!(config.warnings().len(), 1, "{:?}", config.warnings());
    assert!(config.warnings()[0].contains("ExcludedMethods"));
}

#[test]
fn erb_is_rejected_with_a_clear_error() {
    // RuboCop evaluates ERB (spec line 1849); elysium has no Ruby runtime.
    let project = Project::new();
    project.write(".rubocop.yml", "Style/Encoding:\n  Enabled: <%= 1 == 1 %>\n");
    let err = project.loader().load(Some(Path::new(".rubocop.yml"))).unwrap_err();
    assert!(matches!(err, ConfigError::ErbUnsupported(_)), "{err}");
    assert!(err.to_string().contains("ERB"));
}

#[test]
fn malformed_and_missing_files_are_reported() {
    // spec/rubocop/config_loader_spec.rb:1878 and :1999
    let project = Project::new();
    project.write(".rubocop.yml", "This string is not a YAML hash\n");
    let err = project.loader().load(Some(Path::new(".rubocop.yml"))).unwrap_err();
    assert!(matches!(err, ConfigError::Malformed(_)), "{err}");
    assert!(err.to_string().starts_with("Malformed configuration in"));

    let err = project.loader().load(Some(Path::new("nope.yml"))).unwrap_err();
    assert!(matches!(err, ConfigError::NotFound(_)), "{err}");

    // An empty file is an empty configuration (spec line 1895). The malformed
    // dotfile must go first: RuboCop also loads the project's outermost
    // `.rubocop.yml` for its `AllCops/Exclude`.
    project.write(".rubocop.yml", "AllCops:\n  Exclude:\n    - 'ignored/**/*'\n");
    project.write("empty.yml", "");
    let config = project.loader().load(Some(Path::new("empty.yml"))).expect("empty loads");
    assert!(enabled(&config, "Layout/TrailingWhitespace"));
}

#[test]
fn inherit_gem_resolves_through_a_fake_gem_home() {
    // spec/rubocop/config_loader_spec.rb:1266 "inherits from a known gem"
    let project = Project::new();
    let gem_home = project.path().join("fake_gem_home");
    std::fs::create_dir_all(gem_home.join("gems/gitlab-styles-1.2.0")).expect("mkdir");
    std::fs::write(
        gem_home.join("gems/gitlab-styles-1.2.0/rubocop-default.yml"),
        "Style/Alias:\n  Enabled: false\nStyle/For:\n  Exclude:\n    - 'gem_relative.rb'\n",
    )
    .expect("write gem config");
    project.write("Gemfile.lock", "GEM\n  specs:\n    gitlab-styles (1.2.0)\n");
    project.write(
        ".rubocop.yml",
        "inherit_gem:\n  gitlab-styles:\n    - rubocop-default.yml\nStyle/AndOr:\n  Enabled: false\n",
    );

    let loader = project.loader().with_gem_roots(vec![gem_home.clone()]);
    let config = loader.load(Some(Path::new(".rubocop.yml"))).expect("gem config loads");
    assert!(!enabled(&config, "Style/Alias"));
    assert!(!enabled(&config, "Style/AndOr"));
    assert_eq!(
        config.cop("Style/For").unwrap().exclude,
        [project.abs("gem_relative.rb")],
        "a gem config file is not named `.rubocop*`, so RuboCop anchors its paths at the \
         working directory (Config#base_dir_for_path_parameters)"
    );
}

#[test]
fn missing_gem_names_the_gem_and_searched_paths() {
    // spec/rubocop/config_loader_spec.rb:1236 "inherits from an unknown gem"
    let project = Project::new();
    project.write(".rubocop.yml", "inherit_gem:\n  not_there:\n    - config/rubocop.yml\n");
    let err = project.loader().load(Some(Path::new(".rubocop.yml"))).unwrap_err();
    match &err {
        ConfigError::GemNotFound { gem, .. } => assert_eq!(gem, "not_there"),
        other => panic!("expected GemNotFound, got {other}"),
    }
    assert!(err.to_string().contains("Unable to find gem not_there"));
}

#[test]
fn inherit_gem_rubocop_is_rejected() {
    // spec/rubocop/config_loader_spec.rb:1251
    let project = Project::new();
    project.write(".rubocop.yml", "inherit_gem:\n  rubocop:\n    - config/default.yml\n");
    let err = project.loader().load(Some(Path::new(".rubocop.yml"))).unwrap_err();
    assert!(matches!(err, ConfigError::InheritFromRubocopGem), "{err}");
}

#[test]
fn glob_inheritance_loads_every_match() {
    // spec/rubocop/config_loader_spec.rb:516 "inherits from multiple files using a glob"
    let project = Project::new();
    project.write(".rubocop_a.yml", "Style/Alias:\n  Enabled: false\n");
    project.write(".rubocop_b.yml", "Style/AndOr:\n  Enabled: false\n");
    project.write(".rubocop.yml", "inherit_from:\n  - '.rubocop_?.yml'\n");

    let config = project.load(".rubocop.yml");
    assert!(!enabled(&config, "Style/Alias"));
    assert!(!enabled(&config, "Style/AndOr"));
}

#[test]
fn remote_inheritance_is_rejected() {
    let project = Project::new();
    project.write(".rubocop.yml", "inherit_from: https://example.com/rubocop.yml\n");
    let err = project.loader().load(Some(Path::new(".rubocop.yml"))).unwrap_err();
    assert!(matches!(err, ConfigError::RemoteUnsupported { .. }), "{err}");
}

#[test]
fn circular_inheritance_is_detected() {
    let project = Project::new();
    project.write("a.yml", "inherit_from: b.yml\n");
    project.write("b.yml", "inherit_from: a.yml\n");
    let err = project.loader().load(Some(Path::new("a.yml"))).unwrap_err();
    assert!(matches!(err, ConfigError::CircularInheritance(_)), "{err}");
}

#[test]
fn nil_values_remove_keys_when_merging_with_the_defaults() {
    // spec/rubocop/config_loader_spec.rb:563 "inherits and overrides a hash with nil"
    let project = Project::new();
    project.write(
        ".rubocop_parent.yml",
        "Style/InverseMethods:\n  InverseMethods:\n    :any?: :none?\n    :<: :>=\n",
    );
    project.write(
        ".rubocop.yml",
        concat!(
            "inherit_from: .rubocop_parent.yml\n",
            "Style/InverseMethods:\n  InverseMethods:\n    :<: ~\n    :foo: :bar\n",
        ),
    );
    let config = project.load(".rubocop.yml");
    let methods = config
        .cop("Style/InverseMethods")
        .unwrap()
        .options
        .get("InverseMethods")
        .and_then(config::YamlValue::as_mapping)
        .expect("InverseMethods hash");
    assert!(!methods.contains_key(":<"), "nil values remove inherited keys");
    assert_eq!(methods.get_str(":foo"), Some(":bar"));
    assert_eq!(methods.get_str(":any?"), Some(":none?"));
    // Keys from the bundled defaults survive.
    assert!(methods.contains_key(":=~"), "{:?}", methods.keys().collect::<Vec<_>>());

    // A whole cop set to `~` keeps running with an empty configuration, the
    // way `Config#for_cop` reports it.
    project.write(".rubocop.yml", "Style/For: ~\n");
    let config = project.load(".rubocop.yml");
    let cop = config.cop("Style/For").expect("cop is still known");
    assert!(cop.enabled);
    assert!(!cop.options.contains_key("EnforcedStyle"));
}

#[test]
fn all_cops_exclude_from_a_higher_level_file_applies() {
    // ConfigLoader.add_excludes_from_files
    let project = Project::new();
    project.write("Gemfile", "");
    project.write(".rubocop.yml", "AllCops:\n  Exclude:\n    - 'top_level/**/*'\n");
    project.write("subdir/.rubocop.yml", "AllCops:\n  Exclude:\n    - 'sub/**/*'\n");

    let config = project.load("subdir/.rubocop.yml");
    let excludes = &config.all_cops().exclude;
    assert!(excludes.contains(&project.abs("top_level/**/*")), "{excludes:?}");
    assert!(excludes.contains(&project.abs("subdir/sub/**/*")), "{excludes:?}");

    let ignoring = project
        .loader()
        .ignore_parent_exclusion(true)
        .load(Some(Path::new("subdir/.rubocop.yml")))
        .expect("loads");
    assert!(!ignoring.all_cops().exclude.contains(&project.abs("top_level/**/*")));
}

#[test]
fn find_config_file_prefers_the_closest_then_home_then_xdg() {
    // spec/rubocop/config_loader_spec.rb:24 ".configuration_file_for"
    let project = Project::new();
    project.write("dir/example.rb", "");
    assert_eq!(project.loader().find_config_file(&project.path().join("dir")), None);

    project.write("home/.config/rubocop/config.yml", "");
    assert_eq!(
        project.loader().find_config_file(&project.path().join("dir")),
        Some(project.path().join("home/.config/rubocop/config.yml"))
    );

    project.write("home/.rubocop.yml", "");
    assert_eq!(
        project.loader().find_config_file(&project.path().join("dir")),
        Some(project.path().join("home/.rubocop.yml")),
        "the home dotfile beats the XDG config"
    );

    project.write(".config/.rubocop.yml", "");
    assert_eq!(
        project.loader().find_config_file(&project.path().join("dir")),
        Some(project.path().join(".config/.rubocop.yml")),
        "the project's .config copy beats the home dotfile"
    );

    project.write(".rubocop.yml", "");
    assert_eq!(
        project.loader().find_config_file(&project.path().join("dir")),
        Some(project.path().join(".rubocop.yml"))
    );

    project.write("dir/.rubocop.yml", "");
    assert_eq!(
        project.loader().find_config_file(&project.path().join("dir")),
        Some(project.path().join("dir/.rubocop.yml")),
        "the closest file wins"
    );
}

#[test]
fn file_matching_honours_all_cops_and_cop_clusivity() {
    let project = Project::new();
    project.write(
        ".rubocop.yml",
        concat!(
            "AllCops:\n  Exclude:\n    - 'db/**/*'\n",
            "Style/Documentation:\n  Include:\n    - 'app/**/*.rb'\n",
        ),
    );
    let config = project.load(".rubocop.yml");
    assert!(config.file_matcher().is_target(Path::new("app/user.rb")));
    assert!(!config.file_matcher().is_target(Path::new("db/schema.rb")));
    assert!(config.cop_file_matcher("Style/Documentation").is_some());
    assert!(config.cop_file_matcher("Style/Alias").is_none());
    assert!(config.is_cop_enabled_for("Style/Documentation", Path::new("app/user.rb")));
    assert!(!config.is_cop_enabled_for("Style/Documentation", Path::new("lib/user.rb")));
    assert!(!config.is_cop_enabled_for("Style/Documentation", Path::new("db/schema.rb")));
    assert!(config.is_cop_enabled_for("Style/Alias", Path::new("lib/user.rb")));
    assert!(!config.is_cop_enabled_for("Style/NotACop", Path::new("lib/user.rb")));
}

#[test]
fn cop_targeting_ignores_all_cops_include_for_already_discovered_files() {
    // `script/discourse`-style extension-less scripts never match `AllCops`'s
    // `Include` (`**/*.rb`, ...); they are only ever selected as lint
    // targets through the discoverer's shebang fallback (see
    // `discover::ruby_shebang`, crates/cli). RuboCop's own
    // `Cop::Base#relevant_file?` never re-checks `AllCops`'s `Include`/
    // `Exclude` per cop, only the cop's own; `is_cop_enabled_for`/
    // `is_cop_targeting` must match that, or every such shebang-only
    // script silently loses every cop that goes through `CopOverride`
    // (`DisabledByDefault`, `--only`, per-cop `Include`/`Exclude`/
    // `Severity`).
    let project = Project::new();
    project.write(
        ".rubocop.yml",
        concat!(
            "AllCops:\n  DisabledByDefault: true\n",
            "Layout/TrailingWhitespace:\n  Enabled: true\n",
        ),
    );
    let config = project.load(".rubocop.yml");
    let relative = Path::new("script/discourse");
    assert!(
        !config.file_matcher().is_target(relative),
        "no extension, so AllCops Include misses it"
    );
    assert!(config.is_cop_enabled_for("Layout/TrailingWhitespace", relative));
    assert!(config.is_cop_targeting("Layout/TrailingWhitespace", relative));
}

#[test]
fn requested_extensions_are_recorded_and_ignored() {
    let project = Project::new();
    project.write(
        ".rubocop.yml",
        "plugins:\n  - rubocop-performance\nrequire:\n  - rubocop-rspec\n  - ./local_cops.rb\n",
    );
    let config = project.load(".rubocop.yml");
    assert_eq!(config.requested_extensions(), ["rubocop-performance", "rubocop-rspec"]);
    assert!(config.required_features().any(|f| f == "./local_cops.rb"));
    // Naming a gem in `plugins` also silences the `extracted` obsoletion.
    project.write(
        ".rubocop.yml",
        "plugins:\n  - rubocop-performance\nPerformance/Count:\n  Enabled: false\n",
    );
    assert!(project.loader().load(Some(Path::new(".rubocop.yml"))).is_ok());
}

#[test]
fn show_cops_output_parses_and_reports_resolved_values() {
    let project = Project::new();
    project.write(
        ".rubocop.yml",
        "AllCops:\n  NewCops: enable\nStyle/Alias:\n  EnforcedStyle: prefer_alias_method\n",
    );
    let config = project.load(".rubocop.yml");

    let mut out = Vec::new();
    config.render_show_cops(&mut out, None).expect("render");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.starts_with("# Available cops ("), "{}", &text[..60]);
    assert!(text.contains("# Department 'Bundler' ("));
    assert!(text.contains("\nStyle/Alias:\n  Description:"));
    assert!(text.contains("  EnforcedStyle: prefer_alias_method\n"));

    let mut out = Vec::new();
    config.render_show_cops(&mut out, Some(&["Style/Ali*"])).expect("render");
    let filtered = String::from_utf8(out).expect("utf8");
    assert!(filtered.starts_with("Style/Alias:\n"));
    assert!(!filtered.contains("Style/AndOr:"));
    assert!(filtered.contains(":\n"));
}

#[test]
fn effective_hash_is_stable_and_sensitive() {
    let project = Project::new();
    project.write(".rubocop.yml", "Style/Alias:\n  Enabled: false\n");
    let first = project.load(".rubocop.yml").effective_hash();
    let second = project.load(".rubocop.yml").effective_hash();
    assert_eq!(first, second);

    project.write(".rubocop.yml", "Style/Alias:\n  Enabled: true\n");
    assert_ne!(first, project.load(".rubocop.yml").effective_hash());

    project.write(".rubocop.yml", "Metrics/MethodLength:\n  Max: 42\n");
    let by_option = project.load(".rubocop.yml").effective_hash();
    project.write(".rubocop.yml", "Metrics/MethodLength:\n  Max: 43\n");
    assert_ne!(by_option, project.load(".rubocop.yml").effective_hash());
}

#[test]
fn cop_source_points_at_the_configuring_file() {
    let project = Project::new();
    let path = project.write(".rubocop.yml", "Style/Alias:\n  Enabled: false\n");
    let config = project.load(".rubocop.yml");
    assert_eq!(config.cop("Style/Alias").unwrap().source.as_deref(), Some(path.as_path()));
    assert_eq!(config.cop("Style/AndOr").unwrap().source, None);
}

#[test]
fn severity_is_resolved_to_the_linter_type() {
    let project = Project::new();
    project.write(".rubocop.yml", "Style/Alias:\n  Severity: warning\n");
    let config = project.load(".rubocop.yml");
    assert_eq!(config.cop("Style/Alias").unwrap().severity, Some(linter::Severity::Warning));
    assert_eq!(
        config.cop("Bundler/DuplicatedGem").unwrap().severity,
        Some(linter::Severity::Warning),
        "default.yml sets Severity: warning for this cop"
    );
    assert_eq!(
        config.cop("Layout/LineLength").unwrap().severity,
        None,
        "cops without a configured Severity keep the rule's own default"
    );
}

/// Throwaway check against a real-world configuration; run with
/// `cargo test -p config -- --ignored gitlab`.
#[test]
#[ignore = "requires the local corpus checkout"]
fn loads_the_gitlab_rubocop_config() {
    let path = Path::new("/Users/paulo/Work/lab/corpus/gitlab/.rubocop.yml");
    let loader = ConfigLoader::new().with_cwd(path.parent().unwrap());
    match loader.load(Some(path)) {
        Ok(config) => {
            println!(
                "loaded: {} cops, {} warnings, extensions {:?}",
                config.cops().count(),
                config.warnings().len(),
                config.requested_extensions()
            );
        }
        Err(err) => println!("ConfigError: {err}"),
    }
}
