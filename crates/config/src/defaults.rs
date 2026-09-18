//! The embedded RuboCop 1.82.1 `config/default.yml`.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use crate::yaml::{parse_document, Mapping, YamlValue};

/// RuboCop 1.82.1 `config/default.yml`, verbatim (see `rubocop/LICENSE.txt`).
pub const DEFAULT_YML: &str = include_str!("../rubocop/default.yml");

/// The parsed default configuration.
pub(crate) static DEFAULT_CONFIG: LazyLock<Mapping> =
    LazyLock::new(|| match parse_document(DEFAULT_YML).expect("embedded default.yml parses") {
        Some(YamlValue::Mapping(map)) => map,
        other => panic!("embedded default.yml is not a mapping: {other:?}"),
    });

/// Every department named by a cop in the default configuration, longest
/// first so nested departments (`Foo/Bar`) win over their prefix.
pub(crate) static DEPARTMENTS: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
    DEFAULT_CONFIG
        .keys()
        .filter_map(|key| key.rsplit_once('/').map(|(dept, _)| dept.to_string()))
        .collect()
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_the_expected_shape() {
        let all_cops = DEFAULT_CONFIG.get_mapping("AllCops").expect("AllCops");
        assert_eq!(all_cops.get_str("NewCops"), Some("pending"));
        assert!(all_cops.get_string_list("Include").contains(&"**/*.rb".to_string()));
        let cop = DEFAULT_CONFIG.get_mapping("Style/Alias").expect("Style/Alias");
        assert_eq!(cop.get_str("EnforcedStyle"), Some("prefer_alias"));
        assert_eq!(cop.get("Enabled"), Some(&YamlValue::Bool(true)));
        // `Lint/AssignmentInCondition: AllowSafeAssignment` is asserted by
        // spec/rubocop/config_loader_spec.rb:2155.
        assert_eq!(
            DEFAULT_CONFIG
                .get_mapping("Lint/AssignmentInCondition")
                .unwrap()
                .get_bool("AllowSafeAssignment"),
            Some(true)
        );
        assert!(DEPARTMENTS.contains("Style"));
        assert!(DEPARTMENTS.contains("Metrics"));
        assert!(DEFAULT_CONFIG.keys().filter(|k| k.contains('/')).count() > 500);
    }
}
