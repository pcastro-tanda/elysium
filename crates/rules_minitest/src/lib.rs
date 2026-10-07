//! Every rubocop-minitest 0.40.0 rule, one file per rule, grouped by department.
//!
//! [`rules_support::rule_set!`] below is the crate's single registration
//! point; the `registry` crate composes it with the other rule crates.
//! [`DEFAULT_YML`] is the gem's own `config/default.yml`, vendored verbatim
//! (see `rubocop-minitest/LICENSE.txt`) for the fixture harness, which resolves each
//! ported case against it the way the gem's own test suite does. At run
//! time the installed gem's copy is read instead, like any `plugins:`
//! entry's.

pub mod minitest;

/// The gem whose cops this crate ports.
pub const GEM: &str = "rubocop-minitest";

/// rubocop-minitest 0.40.0 `config/default.yml`, verbatim.
pub const DEFAULT_YML: &str = include_str!("../rubocop-minitest/default.yml");

rules_support::rule_set! {
    minitest::useless_assertion::UselessAssertion,
    minitest::unspecified_exception::UnspecifiedException,
    minitest::unreachable_assertion::UnreachableAssertion,
    minitest::test_file_name::TestFileName,
    minitest::skip_without_reason::SkipWithoutReason,
    minitest::skip_ensure::SkipEnsure,
    minitest::return_in_test_method::ReturnInTestMethod,
    minitest::refute_same::RefuteSame,
    minitest::refute_respond_to::RefuteRespondTo,
    minitest::refute_predicate::RefutePredicate,
    minitest::refute_path_exists::RefutePathExists,
    minitest::refute_operator::RefuteOperator,
    minitest::refute_nil::RefuteNil,
    minitest::refute_match::RefuteMatch,
    minitest::refute_kind_of::RefuteKindOf,
    minitest::refute_instance_of::RefuteInstanceOf,
    minitest::refute_includes::RefuteIncludes,
    minitest::refute_in_delta::RefuteInDelta,
    minitest::refute_false::RefuteFalse,
    minitest::refute_equal::RefuteEqual,
    minitest::refute_empty::RefuteEmpty,
    minitest::redundant_message_argument::RedundantMessageArgument,
    minitest::non_public_test_method::NonPublicTestMethod,
    minitest::non_executable_test_method::NonExecutableTestMethod,
    minitest::literal_as_actual_argument::LiteralAsActualArgument,
    minitest::lifecycle_hooks_order::LifecycleHooksOrder,
    minitest::global_expectations::GlobalExpectations,
    minitest::focus::Focus,
    minitest::empty_line_before_assertion_methods::EmptyLineBeforeAssertionMethods,
    minitest::duplicate_test_run::DuplicateTestRun,
    minitest::assertion_in_lifecycle_hook::AssertionInLifecycleHook,
    minitest::assert_truthy::AssertTruthy,
    minitest::assert_silent::AssertSilent,
    minitest::assert_same::AssertSame,
    minitest::assert_respond_to::AssertRespondTo,
    minitest::assert_raises_with_regexp_argument::AssertRaisesWithRegexpArgument,
    minitest::assert_raises_compound_body::AssertRaisesCompoundBody,
    minitest::assert_predicate::AssertPredicate,
    minitest::assert_path_exists::AssertPathExists,
    minitest::assert_output::AssertOutput,
    minitest::assert_operator::AssertOperator,
    minitest::assert_match::AssertMatch,
    minitest::assert_kind_of::AssertKindOf,
    minitest::assert_instance_of::AssertInstanceOf,
    minitest::assert_includes::AssertIncludes,
    minitest::assert_in_delta::AssertInDelta,
    minitest::assert_equal::AssertEqual,
    minitest::assert_empty::AssertEmpty,
    minitest::assert_nil::AssertNil,
}
