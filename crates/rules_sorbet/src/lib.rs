//! Every rubocop-sorbet 0.16.0 rule, one file per rule, grouped by department.
//!
//! [`rules_support::rule_set!`] below is the crate's single registration
//! point; the `registry` crate composes it with the other rule crates.
//! [`DEFAULT_YML`] is the gem's own `config/default.yml`, vendored verbatim
//! (see `rubocop-sorbet/LICENSE.txt`) for the fixture harness, which resolves each
//! ported case against it the way the gem's own test suite does. At run
//! time the installed gem's copy is read instead, like any `plugins:`
//! entry's.

pub mod sorbet;

/// The gem whose cops this crate ports.
pub const GEM: &str = "rubocop-sorbet";

/// rubocop-sorbet 0.16.0 `config/default.yml`, verbatim.
pub const DEFAULT_YML: &str = include_str!("../rubocop-sorbet/default.yml");

rules_support::rule_set! {
    sorbet::type_alias_name::TypeAliasName,
    sorbet::true_sigil::TrueSigil,
    sorbet::struct_prop_name::StructPropName,
    sorbet::strict_sigil::StrictSigil,
    sorbet::signature_build_order::SignatureBuildOrder,
    sorbet::setter_return_type::SetterReturnType,
    sorbet::select_by_is_a::SelectByIsA,
    sorbet::runtime_on_failure_depends_on_checked::RuntimeOnFailureDependsOnChecked,
    sorbet::refinement::Refinement,
    sorbet::redundant_t_let_for_literal::RedundantTLetForLiteral,
    sorbet::redundant_t_let::RedundantTLet,
    sorbet::obsolete_strict_memoization::ObsoleteStrictMemoization,
    sorbet::multiple_t_enum_values::MultipleTEnumValues,
    sorbet::keyword_argument_ordering::KeywordArgumentOrdering,
    sorbet::has_sigil::HasSigil,
    sorbet::forbid_untyped_struct_props::ForbidUntypedStructProps,
    sorbet::forbid_t_unsafe::ForbidTUnsafe,
    sorbet::forbid_t_bind_in_assignment::ForbidTBindInAssignment,
    sorbet::forbid_t_any_with_nil::ForbidTAnyWithNil,
    sorbet::forbid_rbi_outside_of_allowed_paths::ForbidRBIOutsideOfAllowedPaths,
    sorbet::forbid_extend_t_sig_helpers_in_shims::ForbidExtendTSigHelpersInShims,
    sorbet::forbid_comparable_t_enum::ForbidComparableTEnum,
    sorbet::false_sigil::FalseSigil,
    sorbet::enforce_single_sigil::EnforceSingleSigil,
    sorbet::enforce_sigil_order::EnforceSigilOrder,
    sorbet::empty_line_after_sig::EmptyLineAfterSig,
    sorbet::constants_from_strings::ConstantsFromStrings,
    sorbet::checked_true_in_signature::CheckedTrueInSignature,
    sorbet::capitalized_type_parameters::CapitalizedTypeParameters,
    sorbet::buggy_obsolete_strict_memoization::BuggyObsoleteStrictMemoization,
    sorbet::block_method_definition::BlockMethodDefinition,
    sorbet::binding_constant_without_type_alias::BindingConstantWithoutTypeAlias,
    sorbet::allow_incompatible_override::AllowIncompatibleOverride,
    sorbet::forbid_superclass_const_literal::ForbidSuperclassConstLiteral,
}
