//! Hash merging, `inherit_mode` and department-override semantics, ported from
//! `lib/rubocop/config_loader_resolver.rb`.

use crate::yaml::{Mapping, YamlValue};

/// Options for [`merge`], mirroring the Ruby keyword arguments.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MergeOpts<'a> {
    /// The `inherit_mode` mapping in effect for this merge (the "root mode").
    pub inherit_mode: Option<&'a Mapping>,
    /// When true, a `nil` value in the derived hash deletes the key.
    pub unset_nil: bool,
}

/// `ConfigLoaderResolver#merge`: recursive hash merge with array unions driven
/// by `inherit_mode`.
pub(crate) fn merge(base: &Mapping, derived: &Mapping, opts: MergeOpts<'_>) -> Mapping {
    let mut result = base.merged_with(derived);
    for key in base.keys() {
        let Some(derived_value) = derived.get(key) else { continue };
        let base_value = base.get(key).expect("key comes from base");
        if opts.unset_nil && derived_value.is_null() {
            result.remove(key);
        } else if let (YamlValue::Mapping(b), YamlValue::Mapping(d)) = (base_value, derived_value) {
            result.insert(key, YamlValue::Mapping(merge(b, d, opts)));
        } else if should_union(derived, base, opts.inherit_mode, key) {
            result.insert(key, YamlValue::Array(union(base_value, derived_value)));
        }
    }
    result
}

/// Ruby's `Array(base) | Array(derived)`.
fn union(base: &YamlValue, derived: &YamlValue) -> Vec<YamlValue> {
    let mut out = base.to_array();
    for item in derived.to_array() {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

/// `ConfigLoaderResolver#should_union?`.
fn should_union(derived: &Mapping, base: &Mapping, root_mode: Option<&Mapping>, key: &str) -> bool {
    let is_array = |m: &Mapping| matches!(m.get(key), Some(YamlValue::Array(_)));
    if !is_array(base) && !is_array(derived) {
        return false;
    }
    let derived_mode = derived.get_mapping("inherit_mode");
    if lists(derived_mode, "override", key) {
        return false;
    }
    if lists(derived_mode, "merge", key) {
        return true;
    }
    let base_mode = base.get_mapping("inherit_mode");
    if lists(base_mode, "override", key) {
        return false;
    }
    if lists(base_mode, "merge", key) {
        return true;
    }
    lists(root_mode, "merge", key)
}

fn lists(mode: Option<&Mapping>, which: &str, key: &str) -> bool {
    mode.is_some_and(|m| m.get_string_list(which).iter().any(|k| k == key))
}

/// `ConfigLoaderResolver#determine_inherit_mode`.
pub(crate) fn inherit_mode_for<'a>(hash: &'a Mapping, key: &str) -> Option<&'a Mapping> {
    hash.get_mapping(key)
        .and_then(|cop| cop.get_mapping("inherit_mode"))
        .or_else(|| hash.get_mapping("inherit_mode"))
}

/// True when `hash` disables `department` outright.
pub(crate) fn department_disabled(hash: &Mapping, department: &str) -> bool {
    hash.get_mapping(department).and_then(|d| d.get("Enabled")).and_then(YamlValue::as_bool)
        == Some(false)
}

/// `ConfigLoaderResolver#override_department_setting_for_cops`: an explicit
/// `Enabled: true` for a cop beats its department being disabled.
pub(crate) fn override_department_setting_for_cops(base: &Mapping, derived: &mut Mapping) {
    let keys: Vec<String> = derived.keys().map(str::to_string).collect();
    for key in keys {
        let Some((department, _)) = key.rsplit_once('/') else { continue };
        if !(department_disabled(derived, department) || department_disabled(base, department)) {
            continue;
        }
        if let Some(cop) = derived.get_mapping_mut(&key) {
            if cop.get("Enabled").is_some_and(YamlValue::is_truthy) {
                cop.insert("Enabled", YamlValue::String("override_department".to_string()));
            }
        }
    }
}

/// `ConfigLoaderResolver#override_enabled_for_disabled_departments`: a cop
/// enabled in the base config is disabled when the derived config disables its
/// department.
pub(crate) fn override_enabled_for_disabled_departments(base: &Mapping, derived: &mut Mapping) {
    let mut cops_to_disable: Vec<String> = Vec::new();
    for key in derived.keys() {
        if !department_disabled(derived, key) {
            continue;
        }
        let prefix = format!("{key}/");
        cops_to_disable.extend(base.keys().filter(|k| k.starts_with(&prefix)).map(str::to_string));
    }
    for cop_name in cops_to_disable {
        let enabled_in_base =
            base.get_mapping(&cop_name).and_then(|c| c.get("Enabled")).and_then(YamlValue::as_bool)
                == Some(true);
        if !enabled_in_base {
            continue;
        }
        let mut patch = Mapping::new();
        let mut disabled = Mapping::new();
        disabled.insert("Enabled", YamlValue::Bool(false));
        patch.insert(cop_name, YamlValue::Mapping(disabled));
        *derived = merge(&patch, derived, MergeOpts::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml::{parse_document, YamlValue};

    fn mapping(source: &str) -> Mapping {
        match parse_document(source).unwrap().unwrap() {
            YamlValue::Mapping(m) => m,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn merges_recursively() {
        // spec/rubocop/config_loader_spec.rb:2084 ".merge"
        let base = mapping("AllCops:\n  Include: ['**/*.gemspec']\n  Exclude: []\n");
        let derived = mapping("AllCops:\n  Exclude: ['example.rb']\n");
        let merged = merge(&base, &derived, MergeOpts::default());
        let all_cops = merged.get_mapping("AllCops").unwrap();
        assert_eq!(all_cops.get_string_list("Include"), ["**/*.gemspec"]);
        assert_eq!(all_cops.get_string_list("Exclude"), ["example.rb"]);
    }

    #[test]
    fn unset_nil_deletes_the_key() {
        // spec/rubocop/config_loader_spec.rb:563 "inherits and overrides a hash with nil"
        let base = mapping("Style/For:\n  Exclude: ['a.rb']\n");
        let derived = mapping("Style/For: ~\n");
        let merged = merge(&base, &derived, MergeOpts { inherit_mode: None, unset_nil: true });
        assert!(!merged.contains_key("Style/For"));
        let merged = merge(&base, &derived, MergeOpts::default());
        assert!(merged.get("Style/For").unwrap().is_null());
    }

    #[test]
    fn inherit_mode_merge_unions_arrays_and_override_wins() {
        let base = mapping("Style/For:\n  Exclude: ['a.rb', 'b.rb']\n");
        let derived =
            mapping("inherit_mode:\n  merge:\n    - Exclude\nStyle/For:\n  Exclude: ['c.rb']\n");
        let mode = derived.get_mapping("inherit_mode");
        let merged = merge(&base, &derived, MergeOpts { inherit_mode: mode, unset_nil: false });
        assert_eq!(
            merged.get_mapping("Style/For").unwrap().get_string_list("Exclude"),
            ["a.rb", "b.rb", "c.rb"]
        );

        // A per-cop `override` beats the global `merge` (spec line 661).
        let derived = mapping(concat!(
            "inherit_mode:\n  merge:\n    - Exclude\n",
            "Style/For:\n  inherit_mode:\n    override:\n      - Exclude\n  Exclude: ['c.rb']\n",
        ));
        let mode = derived.get_mapping("inherit_mode");
        let merged = merge(&base, &derived, MergeOpts { inherit_mode: mode, unset_nil: false });
        assert_eq!(merged.get_mapping("Style/For").unwrap().get_string_list("Exclude"), ["c.rb"]);
    }

    #[test]
    fn union_coerces_bare_strings_to_arrays() {
        // spec/rubocop/config_loader_spec.rb:769 InheritedStringSpecifiedArray
        let base = mapping("Naming/VariableNumber:\n  Param: 'bare string'\n");
        let derived = mapping(concat!(
            "Naming/VariableNumber:\n  inherit_mode:\n    merge:\n      - Param\n",
            "  Param:\n    - 'string in array'\n",
        ));
        let merged = merge(&base, &derived, MergeOpts::default());
        assert_eq!(
            merged.get_mapping("Naming/VariableNumber").unwrap().get_string_list("Param"),
            ["bare string", "string in array"]
        );
    }

    #[test]
    fn department_overrides_mark_cops_enabled_explicitly() {
        let base = mapping("Layout/EndOfLine:\n  Enabled: true\n");
        let mut derived =
            mapping("Layout:\n  Enabled: false\nLayout/LineLength:\n  Enabled: true\n");
        override_department_setting_for_cops(&base, &mut derived);
        assert_eq!(
            derived.get_mapping("Layout/LineLength").unwrap().get("Enabled"),
            Some(&YamlValue::String("override_department".to_string()))
        );
        override_enabled_for_disabled_departments(&base, &mut derived);
        assert_eq!(
            derived.get_mapping("Layout/EndOfLine").unwrap().get("Enabled"),
            Some(&YamlValue::Bool(false))
        );
    }
}
