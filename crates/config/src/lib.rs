//! Configuration. Phase 1 ships only RuboCop's default file selection
//! (`AllCops/Include` and `AllCops/Exclude`); `.rubocop.yml` loading and
//! inheritance arrive in Phase 2 and feed the same [`FileMatcher`].

mod file_matcher;

pub use file_matcher::{FileMatcher, DEFAULT_EXCLUDE, DEFAULT_INCLUDE};
