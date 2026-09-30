//! `Metrics` department.
//!
//! [`util`] holds the shared calculators RuboCop keeps in
//! `RuboCop::Cop::Metrics::Utils` -- code length, cyclomatic and perceived
//! complexity, and ABC size. They live here rather than in `linter` because
//! only cops consume them, exactly as upstream keeps them cop-internal.

pub mod abc_size;
pub mod block_length;
pub mod block_nesting;
pub mod class_length;
pub mod cyclomatic_complexity;
pub mod method_length;
pub mod module_length;
pub mod parameter_lists;
pub mod perceived_complexity;
/// The calculators are engine primitives that land ahead of the `Metrics/*`
/// cops consuming them, so nothing in the crate calls them yet.
#[allow(dead_code, reason = "consumed by the Metrics cops ported on top of it")]
pub(crate) mod util;
