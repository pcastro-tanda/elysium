//! Every rubocop-rails 2.38.0 rule, one file per rule, grouped by department.
//!
//! [`rules_support::rule_set!`] below is the crate's single registration
//! point; the `registry` crate composes it with the other rule crates.
//! [`DEFAULT_YML`] is the gem's own `config/default.yml`, vendored verbatim
//! (see `rubocop-rails/LICENSE.txt`) for the fixture harness, which resolves each
//! ported case against it the way the gem's own test suite does. At run
//! time the installed gem's copy is read instead, like any `plugins:`
//! entry's.

pub mod active_record_helper;
pub mod inflector;
pub mod rails;
pub mod schema;

/// The gem whose cops this crate ports.
pub const GEM: &str = "rubocop-rails";

/// rubocop-rails 2.38.0 `config/default.yml`, verbatim.
pub const DEFAULT_YML: &str = include_str!("../rubocop-rails/default.yml");

rules_support::rule_set! {
    rails::create_table_with_timestamps::CreateTableWithTimestamps,
    rails::duration_arithmetic::DurationArithmetic,
    rails::redirect_back_or_to::RedirectBackOrTo,
    rails::link_to_blank::LinkToBlank,
    rails::unique_validation_without_index::UniqueValidationWithoutIndex,
    rails::active_record_override::ActiveRecordOverride,
    rails::freeze_time::FreezeTime,
    rails::to_s_with_argument::ToSWithArgument,
    rails::presence::Presence,
    rails::http_status_name_consistency::HttpStatusNameConsistency,
    rails::application_record::ApplicationRecord,
}
