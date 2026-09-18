//! Obsolete configuration detection, ported from
//! `lib/rubocop/config_obsoletion.rb` and `lib/rubocop/config_obsoletion/*.rb`
//! with the rules from the embedded `config/obsoletion.yml`.

use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

use crate::yaml::{parse_document, Mapping, YamlValue};

/// The RuboCop 1.82.1 `config/obsoletion.yml`.
const OBSOLETION_YML: &str = include_str!("../rubocop/obsoletion.yml");

/// A cop that was renamed (possibly only moved to another department).
#[derive(Debug)]
struct Renamed {
    old_name: String,
    new_name: String,
    warning: bool,
}

/// A cop that no longer exists.
#[derive(Debug)]
struct Removed {
    old_name: String,
    reason: Option<String>,
    alternatives: Vec<String>,
}

/// A cop whose functionality moved into several cops.
#[derive(Debug)]
struct Split {
    old_name: String,
    alternatives: Vec<String>,
}

/// A cop or department that now lives in an extension gem.
#[derive(Debug)]
struct Extracted {
    /// `Performance/*` or a single cop name.
    old_name: String,
    department: String,
    gem: String,
}

/// A renamed or removed cop parameter.
#[derive(Debug)]
struct ParameterRule {
    cop: String,
    parameter: String,
    /// Set for `changed_enforced_styles`, where only one value is obsolete.
    value: Option<String>,
    alternative: Option<String>,
    alternatives: Vec<String>,
    reason: Option<String>,
    warning: bool,
    minimum_ruby_version: Option<f32>,
}

/// Every obsoletion rule RuboCop ships.
#[derive(Debug, Default)]
pub(crate) struct Obsoletion {
    renamed: Vec<Renamed>,
    removed: Vec<Removed>,
    split: Vec<Split>,
    extracted: Vec<Extracted>,
    parameters: Vec<ParameterRule>,
    legacy_names: HashSet<String>,
}

/// Messages produced for one configuration hash.
#[derive(Debug, Default)]
pub(crate) struct Obsoletions {
    /// Hard failures.
    pub errors: Vec<String>,
    /// `severity: warning` rules, which RuboCop only prints.
    pub warnings: Vec<String>,
}

/// The parsed rule table.
pub(crate) static RULES: LazyLock<Obsoletion> = LazyLock::new(|| {
    let YamlValue::Mapping(map) = parse_document(OBSOLETION_YML)
        .expect("embedded obsoletion.yml parses")
        .expect("embedded obsoletion.yml is not empty")
    else {
        panic!("embedded obsoletion.yml is not a mapping");
    };
    Obsoletion::from_mapping(&map)
});

impl Obsoletion {
    fn from_mapping(map: &Mapping) -> Self {
        let mut this = Self::default();
        if let Some(renamed) = map.get_mapping("renamed") {
            for (old_name, value) in renamed.iter() {
                let (new_name, warning) = match value {
                    YamlValue::String(s) => (s.clone(), false),
                    YamlValue::Mapping(m) => (
                        m.get_str("new_name").unwrap_or_default().to_string(),
                        m.get_str("severity") == Some("warning"),
                    ),
                    _ => continue,
                };
                this.renamed.push(Renamed { old_name: old_name.to_string(), new_name, warning });
            }
        }
        if let Some(removed) = map.get_mapping("removed") {
            for (old_name, value) in removed.iter() {
                if !value.is_truthy() {
                    continue;
                }
                let meta = value.as_mapping();
                this.removed.push(Removed {
                    old_name: old_name.to_string(),
                    reason: meta.and_then(|m| m.get_str("reason")).map(str::to_string),
                    alternatives: meta
                        .map(|m| m.get_string_list("alternatives"))
                        .unwrap_or_default(),
                });
            }
        }
        if let Some(split) = map.get_mapping("split") {
            for (old_name, value) in split.iter() {
                let Some(meta) = value.as_mapping() else { continue };
                this.split.push(Split {
                    old_name: old_name.to_string(),
                    alternatives: meta.get_string_list("alternatives"),
                });
            }
        }
        if let Some(extracted) = map.get_mapping("extracted") {
            for (old_name, value) in extracted.iter() {
                let Some(gem) = value.as_str() else { continue };
                let department =
                    old_name.rsplit_once('/').map_or(old_name, |(dept, _)| dept).to_string();
                this.extracted.push(Extracted {
                    old_name: old_name.to_string(),
                    department,
                    gem: gem.to_string(),
                });
            }
        }
        this.load_parameter_rules(map, "changed_parameters", false);
        this.load_parameter_rules(map, "changed_enforced_styles", true);

        for name in this
            .renamed
            .iter()
            .map(|r| r.old_name.clone())
            .chain(this.removed.iter().map(|r| r.old_name.clone()))
            .chain(this.split.iter().map(|r| r.old_name.clone()))
            .chain(this.extracted.iter().map(|r| r.old_name.clone()))
        {
            this.legacy_names.insert(name);
        }
        this
    }

    fn load_parameter_rules(&mut self, map: &Mapping, key: &str, styles: bool) {
        let Some(YamlValue::Array(entries)) = map.get(key) else { return };
        for entry in entries {
            let Some(meta) = entry.as_mapping() else { continue };
            let cops = meta.get_string_list("cops");
            let parameters = meta.get_string_list("parameters");
            for cop in &cops {
                for parameter in &parameters {
                    self.parameters.push(ParameterRule {
                        cop: cop.clone(),
                        parameter: parameter.clone(),
                        value: if styles {
                            meta.get("value").and_then(YamlValue::scalar_string)
                        } else {
                            None
                        },
                        alternative: meta.get_str("alternative").map(str::to_string),
                        alternatives: meta.get_string_list("alternatives"),
                        reason: meta.get_str("reason").map(str::to_string),
                        warning: meta.get_str("severity") == Some("warning"),
                        minimum_ruby_version: meta
                            .get("minimum_ruby_version")
                            .and_then(YamlValue::as_f32),
                    });
                }
            }
        }
    }

    /// RuboCop's `ConfigObsoletion#deprecated_cop_name?`.
    pub(crate) fn is_deprecated_name(&self, name: &str) -> bool {
        self.legacy_names.contains(name)
    }

    /// Old names of `cop`, for `Config#for_cop`'s deprecated-config merge.
    pub(crate) fn deprecated_names_for(&self, cop: &str) -> Vec<&str> {
        self.renamed.iter().filter(|r| r.new_name == cop).map(|r| r.old_name.as_str()).collect()
    }

    /// Every rule violated by `hash`.
    pub(crate) fn check(
        &self,
        hash: &Mapping,
        path: &Path,
        target_ruby_version: f32,
        loaded_extensions: &HashSet<String>,
    ) -> Obsoletions {
        let mut out = Obsoletions::default();
        let suffix =
            format!("\n(obsolete configuration found in {}, please update it)", path.display());

        for rule in &self.renamed {
            if !violates_cop_rule(hash, &rule.old_name) {
                continue;
            }
            let verb = if moved(&rule.old_name, &rule.new_name) { "moved" } else { "renamed" };
            let message = format!(
                "The `{}` cop has been {verb} to `{}`.{suffix}",
                rule.old_name, rule.new_name
            );
            if rule.warning {
                out.warnings.push(message);
            } else {
                out.errors.push(message);
            }
        }
        for rule in &self.removed {
            if !violates_cop_rule(hash, &rule.old_name) {
                continue;
            }
            let base = format!("The `{}` cop has been removed", rule.old_name);
            let message = if let Some(reason) = &rule.reason {
                format!("{base} since {}.", reason.trim_end())
            } else if rule.alternatives.is_empty() {
                format!("{base}.")
            } else {
                format!("{base}. Please use {} instead.", to_sentence(&rule.alternatives, "and/or"))
            };
            out.errors.push(format!("{message}{suffix}"));
        }
        for rule in &self.split {
            if !violates_cop_rule(hash, &rule.old_name) {
                continue;
            }
            out.errors.push(format!(
                "The `{}` cop has been split into {}.{suffix}",
                rule.old_name,
                to_sentence(&rule.alternatives, "and")
            ));
        }
        for rule in &self.extracted {
            if loaded_extensions.contains(&rule.gem) {
                continue;
            }
            let affected = affected_cops(hash, rule);
            if affected.is_empty() {
                continue;
            }
            let name = if affected.len() > 1 {
                format!("`{}` cops have", rule.department)
            } else {
                format!("`{}` has", affected[0])
            };
            out.errors.push(format!("{name} been extracted to the `{}` gem.{suffix}", rule.gem));
        }
        for rule in &self.parameters {
            if let Some(minimum) = rule.minimum_ruby_version {
                if target_ruby_version < minimum {
                    continue;
                }
            }
            let Some(cop_config) = hash.get_mapping(&rule.cop) else { continue };
            let Some(current) = cop_config.get(&rule.parameter) else { continue };
            if let Some(expected) = &rule.value {
                if current.scalar_string().as_deref() != Some(expected.as_str()) {
                    continue;
                }
            }
            let message = Self::parameter_message(rule, path);
            if rule.warning {
                out.warnings.push(message);
            } else {
                out.errors.push(message);
            }
        }
        out
    }

    fn parameter_message(rule: &ParameterRule, path: &Path) -> String {
        let base = if let Some(value) = &rule.value {
            format!(
                "obsolete `{}: {value}` (for `{}`) found in {}",
                rule.parameter,
                rule.cop,
                path.display()
            )
        } else {
            format!(
                "obsolete parameter `{}` (for `{}`) found in {}",
                rule.parameter,
                rule.cop,
                path.display()
            )
        };
        if let Some(alternative) = &rule.alternative {
            if let Some(value) = &rule.value {
                format!(
                    "{base}\n`{}: {value}` has been renamed to `{}: {}`.",
                    rule.parameter,
                    rule.parameter,
                    alternative.trim_end()
                )
            } else {
                format!(
                    "{base}\n`{}` has been renamed to `{}`.",
                    rule.parameter,
                    alternative.trim_end()
                )
            }
        } else if !rule.alternatives.is_empty() {
            format!(
                "{base}\n`{}` has been renamed to {}.",
                rule.parameter,
                to_sentence(&rule.alternatives, "and/or")
            )
        } else {
            format!("{base}\n{}", rule.reason.as_deref().unwrap_or("").trim_end())
        }
    }
}

impl YamlValue {
    /// Scalar values as text, used for obsoletion comparisons.
    fn scalar_string(&self) -> Option<String> {
        match self {
            YamlValue::String(s) | YamlValue::Regexp(s) => Some(s.clone()),
            YamlValue::Int(i) => Some(i.to_string()),
            YamlValue::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }
}

/// `CopRule#violated?`: the old name, or its bare cop name, is configured.
fn violates_cop_rule(hash: &Mapping, old_name: &str) -> bool {
    if hash.contains_key(old_name) {
        return true;
    }
    let bare = old_name.rsplit_once('/').map_or(old_name, |(_, name)| name);
    hash.contains_key(bare)
}

/// `ExtractedCop#affected_cops`.
fn affected_cops(hash: &Mapping, rule: &Extracted) -> Vec<String> {
    if !rule.old_name.ends_with('*') {
        return if violates_cop_rule(hash, &rule.old_name) {
            vec![rule.old_name.clone()]
        } else {
            Vec::new()
        };
    }
    let prefix = format!("{}/", rule.department);
    hash.keys()
        .filter(|key| *key == rule.department || key.starts_with(&prefix))
        .map(str::to_string)
        .collect()
}

/// `RenamedCop#moved?`: same cop name, different department.
fn moved(old_name: &str, new_name: &str) -> bool {
    match (old_name.rsplit_once('/'), new_name.rsplit_once('/')) {
        (Some((old_dept, old_cop)), Some((new_dept, new_cop))) => {
            old_dept != new_dept && old_cop == new_cop
        }
        _ => false,
    }
}

/// `Rule#to_sentence`, with every element backticked as the rules do.
fn to_sentence(items: &[String], connector: &str) -> String {
    let quoted: Vec<String> = items.iter().map(|i| format!("`{i}`")).collect();
    match quoted.len() {
        0 => String::new(),
        1 => quoted[0].clone(),
        _ => format!(
            "{} {connector} {}",
            quoted[..quoted.len() - 1].join(", "),
            quoted[quoted.len() - 1]
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml::parse_document;

    fn mapping(source: &str) -> Mapping {
        match parse_document(source).unwrap().unwrap() {
            YamlValue::Mapping(m) => m,
            other => panic!("{other:?}"),
        }
    }

    fn check(source: &str) -> Obsoletions {
        RULES.check(&mapping(source), Path::new(".rubocop.yml"), 2.7, &HashSet::new())
    }

    #[test]
    fn reports_split_cops() {
        // spec/rubocop/config_loader_spec.rb:2025 expects `Style/MethodMissing` to raise.
        let out = check("Style/MethodMissing:\n  Enabled: true\n");
        assert_eq!(out.errors.len(), 1);
        assert!(
            out.errors[0].starts_with(
                "The `Style/MethodMissing` cop has been split into \
                 `Style/MethodMissingSuper` and `Style/MissingRespondToMissing`."
            ),
            "{:?}",
            out.errors[0]
        );
        assert!(out.errors[0].contains("(obsolete configuration found in .rubocop.yml"));
    }

    #[test]
    fn reports_renamed_cops_and_moves() {
        let out = check("Lint/Eval:\n  Enabled: true\n");
        assert_eq!(out.errors, ["The `Lint/Eval` cop has been moved to `Security/Eval`.\n(obsolete configuration found in .rubocop.yml, please update it)"]);
        let out = check("Metrics/LineLength:\n  Max: 100\n");
        assert!(out.errors[0]
            .starts_with("The `Metrics/LineLength` cop has been moved to `Layout/LineLength`."));
    }

    #[test]
    fn renamed_with_warning_severity_is_not_an_error() {
        let out = check("Naming/PredicateName:\n  Enabled: true\n");
        assert!(out.errors.is_empty(), "{:?}", out.errors);
        assert_eq!(out.warnings.len(), 1);
        assert!(out.warnings[0].contains("`Naming/PredicatePrefix`"));
    }

    #[test]
    fn reports_extracted_departments_once() {
        let out = check("Rails/Date:\n  Enabled: true\nRails/TimeZone:\n  Enabled: false\n");
        assert_eq!(out.errors.len(), 1);
        assert_eq!(
            out.errors[0],
            "`Rails` cops have been extracted to the `rubocop-rails` gem.\n\
             (obsolete configuration found in .rubocop.yml, please update it)"
        );
    }

    #[test]
    fn extracted_rule_is_skipped_when_the_plugin_is_loaded() {
        let loaded = HashSet::from(["rubocop-rails".to_string()]);
        let out = RULES.check(
            &mapping("Rails/Date:\n  Enabled: true\n"),
            Path::new(".rubocop.yml"),
            2.7,
            &loaded,
        );
        assert!(out.errors.is_empty(), "{:?}", out.errors);
    }

    #[test]
    fn reports_removed_cops_with_alternatives() {
        let out = check("Style/TrailingCommaInLiteral:\n  Enabled: true\n");
        assert_eq!(
            out.errors[0].lines().next().unwrap(),
            "The `Style/TrailingCommaInLiteral` cop has been removed. Please use \
             `Style/TrailingCommaInArrayLiteral` and/or `Style/TrailingCommaInHashLiteral` instead."
        );
    }

    #[test]
    fn reports_changed_parameters() {
        let out = check("Layout/CaseIndentation:\n  IndentWhenRelativeTo: case\n");
        assert_eq!(
            out.errors[0],
            "obsolete parameter `IndentWhenRelativeTo` (for `Layout/CaseIndentation`) \
             found in .rubocop.yml\n`IndentWhenRelativeTo` has been renamed to `EnforcedStyle`."
        );
    }

    #[test]
    fn changed_parameter_with_minimum_ruby_version_respects_target() {
        let hash = mapping("Style/ArgumentsForwarding:\n  AllowOnlyRestArgument: true\n");
        let path = Path::new(".rubocop.yml");
        let out = RULES.check(&hash, path, 3.1, &HashSet::new());
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        let out = RULES.check(&hash, path, 3.2, &HashSet::new());
        assert_eq!(out.warnings.len(), 1);
    }

    #[test]
    fn changed_enforced_styles_only_fire_for_the_obsolete_value() {
        let out = check("Layout/IndentationConsistency:\n  EnforcedStyle: rails\n");
        assert_eq!(
            out.errors[0],
            "obsolete `EnforcedStyle: rails` (for `Layout/IndentationConsistency`) found in \
             .rubocop.yml\n`EnforcedStyle: rails` has been renamed to \
             `EnforcedStyle: indented_internal_methods`."
        );
        let out = check("Layout/IndentationConsistency:\n  EnforcedStyle: normal\n");
        assert!(out.errors.is_empty(), "{:?}", out.errors);
    }

    #[test]
    fn bare_cop_names_violate_rules_too() {
        let out = check("MethodMissing:\n  Enabled: true\n");
        assert_eq!(out.errors.len(), 1);
    }
}
