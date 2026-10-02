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
    rails::after_commit_override::AfterCommitOverride,
    rails::uniq_before_pluck::UniqBeforePluck,
    rails::eager_evaluation_log_message::EagerEvaluationLogMessage,
    rails::find_by_id::FindById,
    rails::where_range::WhereRange,
    rails::redundant_foreign_key::RedundantForeignKey,
    rails::find_each::FindEach,
    rails::render_plain_text::RenderPlainText,
    rails::find_by::FindBy,
    rails::strong_parameters_expect::StrongParametersExpect,
    rails::squished_sql_heredocs::SquishedSQLHeredocs,
    rails::reflection_class_name::ReflectionClassName,
    rails::enum_uniqueness::EnumUniqueness,
    rails::order_arguments::OrderArguments,
    rails::redundant_active_record_all_method::RedundantActiveRecordAllMethod,
    rails::output::Output,
    rails::pick::Pick,
    rails::unused_render_content::UnusedRenderContent,
    rails::exit::Exit,
    rails::inverse_of::InverseOf,
    rails::root_join_chain::RootJoinChain,
    rails::three_state_boolean_column::ThreeStateBooleanColumn,
    rails::dot_separated_keys::DotSeparatedKeys,
    rails::short_i18n::ShortI18n,
    rails::lexically_scoped_action_filter::LexicallyScopedActionFilter,
    rails::negate_include::NegateInclude,
    rails::render_inline::RenderInline,
    rails::assert_not::AssertNot,
    rails::file_path::FilePath,
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
    rails::redundant_travel_back::RedundantTravelBack,
    rails::strip_heredoc::StripHeredoc,
    rails::request_referer::RequestReferer,
    rails::multiple_route_paths::MultipleRoutePaths,
    rails::time_zone::TimeZone,
    rails::migration_class_name::MigrationClassName,
    rails::add_column_index::AddColumnIndex,
    rails::active_support_aliases::ActiveSupportAliases,
    rails::root_public_path::RootPublicPath,
    rails::bulk_change_table::BulkChangeTable,
    rails::application_record::ApplicationRecord,
}
