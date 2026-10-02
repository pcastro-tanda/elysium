//! `TargetRailsVersion`, shared by the cops that declare
//! `minimum_target_rails_version`.

use linter::{OptionValue, RuleOptions};

/// `TargetRailsVersion::DEFAULT_RAILS_VERSION`, used when the configuration
/// states none.
const DEFAULT_RAILS_VERSION: f64 = 5.0;

/// `Config#target_rails_version`: `AllCops/TargetRailsVersion` when set.
pub(super) fn target_rails_version(options: &RuleOptions) -> f64 {
    match options.peer("AllCops", "TargetRailsVersion") {
        Some(OptionValue::Str(text)) => text.trim().parse().unwrap_or(DEFAULT_RAILS_VERSION),
        Some(value) => value.as_float().unwrap_or(DEFAULT_RAILS_VERSION),
        None => DEFAULT_RAILS_VERSION,
    }
}
