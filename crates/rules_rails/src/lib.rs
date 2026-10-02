//! Every rubocop-rails 2.38.0 rule, one file per rule, grouped by department.
//!
//! [`rules_support::rule_set!`] below is the crate's single registration
//! point; the `registry` crate composes it with the other rule crates.
//! [`DEFAULT_YML`] is the gem's own `config/default.yml`, vendored verbatim
//! (see `rubocop-rails/LICENSE.txt`) for the fixture harness, which resolves each
//! ported case against it the way the gem's own test suite does. At run
//! time the installed gem's copy is read instead, like any `plugins:`
//! entry's.

pub mod rails;

/// The gem whose cops this crate ports.
pub const GEM: &str = "rubocop-rails";

/// rubocop-rails 2.38.0 `config/default.yml`, verbatim.
pub const DEFAULT_YML: &str = include_str!("../rubocop-rails/default.yml");

rules_support::rule_set! {
    rails::where_missing::WhereMissing,
    rails::where_equals::WhereEquals,
    rails::duplicate_association::DuplicateAssociation,
    rails::pluralization_grammar::PluralizationGrammar,
    rails::present::Present,
    rails::application_record::ApplicationRecord,
}
