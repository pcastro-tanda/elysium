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
    rails::delegate_allow_blank::DelegateAllowBlank,
    rails::application_controller::ApplicationController,
    rails::scope_args::ScopeArgs,
    rails::has_and_belongs_to_many::HasAndBelongsToMany,
    rails::dangerous_column_names::DangerousColumnNames,
    rails::inquiry::Inquiry,
    rails::application_mailer::ApplicationMailer,
    rails::time_zone_assignment::TimeZoneAssignment,
    rails::i18n_locale_assignment::I18nLocaleAssignment,
    rails::reversible_migration::ReversibleMigration,
    rails::safe_navigation_with_blank::SafeNavigationWithBlank,
    rails::top_level_hash_with_indifferent_access::TopLevelHashWithIndifferentAccess,
    rails::action_controller_test_case::ActionControllerTestCase,
    rails::active_record_aliases::ActiveRecordAliases,
    rails::redundant_presence_validation_on_belongs_to::RedundantPresenceValidationOnBelongsTo,
    rails::arel_star::ArelStar,
    rails::to_formatted_s::ToFormattedS,
    rails::duplicate_scope::DuplicateScope,
    rails::application_job::ApplicationJob,
    rails::root_pathname_methods::RootPathnameMethods,
    rails::application_record::ApplicationRecord,
}
