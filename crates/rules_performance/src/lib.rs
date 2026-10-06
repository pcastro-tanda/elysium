//! Every rubocop-performance 1.27.0 rule, one file per rule, grouped by department.
//!
//! [`rules_support::rule_set!`] below is the crate's single registration
//! point; the `registry` crate composes it with the other rule crates.
//! [`DEFAULT_YML`] is the gem's own `config/default.yml`, vendored verbatim
//! (see `rubocop-performance/LICENSE.txt`) for the fixture harness, which resolves each
//! ported case against it the way the gem's own test suite does. At run
//! time the installed gem's copy is read instead, like any `plugins:`
//! entry's.

pub mod performance;

/// The gem whose cops this crate ports.
pub const GEM: &str = "rubocop-performance";

/// rubocop-performance 1.27.0 `config/default.yml`, verbatim.
pub const DEFAULT_YML: &str = include_str!("../rubocop-performance/default.yml");

rules_support::rule_set! {
    performance::caller::Caller,
}
