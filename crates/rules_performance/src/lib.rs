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
    performance::zip_without_block::ZipWithoutBlock,
    performance::uri_default_parser::UriDefaultParser,
    performance::times_map::TimesMap,
    performance::sum::Sum,
    performance::string_replacement::StringReplacement,
    performance::string_include::StringInclude,
    performance::string_identifier_argument::StringIdentifierArgument,
    performance::string_bytesize::StringBytesize,
    performance::start_with::StartWith,
    performance::squeeze::Squeeze,
    performance::sort_reverse::SortReverse,
    performance::size::Size,
    performance::reverse_first::ReverseFirst,
    performance::reverse_each::ReverseEach,
    performance::regexp_match::RegexpMatch,
    performance::redundant_string_chars::RedundantStringChars,
    performance::redundant_split_regexp_argument::RedundantSplitRegexpArgument,
    performance::redundant_sort_block::RedundantSortBlock,
    performance::redundant_merge::RedundantMerge,
    performance::redundant_match::RedundantMatch,
    performance::redundant_equality_comparison_block::RedundantEqualityComparisonBlock,
    performance::range_include::RangeInclude,
    performance::method_object_as_block::MethodObjectAsBlock,
    performance::map_method_chain::MapMethodChain,
    performance::map_compact::MapCompact,
    performance::inefficient_hash_search::InefficientHashSearch,
    performance::flat_map::FlatMap,
    performance::fixed_size::FixedSize,
    performance::end_with::EndWith,
    performance::double_start_end_with::DoubleStartEndWith,
    performance::delete_suffix::DeleteSuffix,
    performance::delete_prefix::DeletePrefix,
    performance::constant_regexp::ConstantRegexp,
    performance::concurrent_monotonic_time::ConcurrentMonotonicTime,
    performance::compare_with_block::CompareWithBlock,
    performance::case_when_splat::CaseWhenSplat,
    performance::block_given_with_explicit_block::BlockGivenWithExplicitBlock,
    performance::bind_call::BindCall,
    performance::big_decimal_with_numeric_argument::BigDecimalWithNumericArgument,
    performance::ancestors_include::AncestorsInclude,
    performance::caller::Caller,
}
