//! Every rubocop-thread_safety 0.8.0 rule, one file per rule, grouped by department.
//!
//! [`rules_support::rule_set!`] below is the crate's single registration
//! point; the `registry` crate composes it with the other rule crates.
//! [`DEFAULT_YML`] is the gem's own `config/default.yml`, vendored verbatim
//! (see `rubocop-thread_safety/LICENSE.txt`) for the fixture harness, which resolves each
//! ported case against it the way the gem's own test suite does. At run
//! time the installed gem's copy is read instead, like any `plugins:`
//! entry's.

pub mod thread_safety;

/// The gem whose cops this crate ports.
pub const GEM: &str = "rubocop-thread_safety";

/// rubocop-thread_safety 0.8.0 `config/default.yml`, verbatim.
pub const DEFAULT_YML: &str = include_str!("../rubocop-thread_safety/default.yml");

rules_support::rule_set! {
    thread_safety::new_thread::NewThread,
}
