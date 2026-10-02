//! Every rule, one file per rule, grouped by RuboCop department.
//!
//! [`rule_set!`] is the single registration point: it expands to the slot
//! list [`RuleSet`] holds and to [`ALL_RULES`]. Node-kind subscription is a
//! compile-time `const` table per rule, and dispatch is a monomorphized
//! walk over the slot list, so no rule is ever behind a `dyn` pointer.

use std::sync::Arc;

use config::{CopConfig, LoadedConfig, YamlValue};
use linter::{
    subscription_table, Context, Diagnostic, Dispatch, OptionError, OptionValue, PeerOptions, Rule,
    RuleMeta, RuleOptions,
};
use ruby_ast::{Node, NodeKind};

pub mod bundler;
pub mod gemspec;
pub mod layout;
pub mod lint;
pub mod metrics;
pub mod migration;
pub mod naming;
pub mod security;
pub mod style;

mod name_similarity;

/// Registers every rule in one place.
///
/// Expands to the private slot-list type [`RuleSet`] stores its configured
/// rules in and to [`ALL_RULES`], in registration order. The slot list is a
/// tree of tuples with `Option<Rule>` leaves, so no identifier has to be
/// synthesized for each rule and cop names that share a snake-case file
/// name across departments -- `Layout/LineLength` and `Metrics/LineLength`
/// -- cannot collide. Rules are grouped 32 to a balanced subtree before
/// chaining (eight at the chain's end), keeping type nesting -- and rustc's
/// drop-check/auto-trait recursion over it, which counts against each
/// downstream crate's `recursion_limit` -- at roughly a sixteenth of the
/// rule count. Each link's tail is boxed: `build` and `clone` recurse once
/// per link and return their subtree by value, so an unboxed chain needs
/// stack quadratic in the rule count in unoptimized builds (past ~500 rules
/// it overflowed the test threads' 2 MiB).
macro_rules! rule_set {
    ($($rule:path),+ $(,)?) => {
        /// Every registered rule's metadata, in registration order.
        pub const ALL_RULES: &[&'static RuleMeta] = &[$(<$rule as RuleExt>::META_REF),+];

        /// The configured rules [`RuleSet`] dispatches to.
        type Slots = rule_set!(@slots $($rule),+);
    };
    (@slots
        $a0:path, $a1:path, $a2:path, $a3:path, $a4:path, $a5:path, $a6:path, $a7:path,
        $b0:path, $b1:path, $b2:path, $b3:path, $b4:path, $b5:path, $b6:path, $b7:path,
        $c0:path, $c1:path, $c2:path, $c3:path, $c4:path, $c5:path, $c6:path, $c7:path,
        $d0:path, $d1:path, $d2:path, $d3:path, $d4:path, $d5:path, $d6:path, $d7:path,
        $($rest:path),+
    ) => {
        (
            (
                (
                    rule_set!(@eight $a0, $a1, $a2, $a3, $a4, $a5, $a6, $a7),
                    rule_set!(@eight $b0, $b1, $b2, $b3, $b4, $b5, $b6, $b7),
                ),
                (
                    rule_set!(@eight $c0, $c1, $c2, $c3, $c4, $c5, $c6, $c7),
                    rule_set!(@eight $d0, $d1, $d2, $d3, $d4, $d5, $d6, $d7),
                ),
            ),
            Box<rule_set!(@slots $($rest),+)>,
        )
    };
    (@slots $a:path, $b:path, $c:path, $d:path, $e:path, $f:path, $g:path, $h:path, $($rest:path),+) => {
        (rule_set!(@eight $a, $b, $c, $d, $e, $f, $g, $h), rule_set!(@slots $($rest),+))
    };
    (@slots $head:path) => { Option<$head> };
    (@slots $head:path, $($tail:path),+) => { (Option<$head>, rule_set!(@slots $($tail),+)) };
    (@eight $a:path, $b:path, $c:path, $d:path, $e:path, $f:path, $g:path, $h:path) => {
        (
            ((Option<$a>, Option<$b>), (Option<$c>, Option<$d>)),
            ((Option<$e>, Option<$f>), (Option<$g>, Option<$h>)),
        )
    };
}

rule_set! {
    style::yaml_file_read::YAMLFileRead,
    style::time_now::TimeNow,
    style::tally_method::TallyMethod,
    style::swap_values::SwapValues,
    style::super_with_args_parentheses::SuperWithArgsParentheses,
    style::super_arguments::SuperArguments,
    style::string_chars::StringChars,
    style::single_line_do_end_block::SingleLineDoEndBlock,
    style::send::Send,
    style::select_by_regexp::SelectByRegexp,
    style::select_by_range::SelectByRange,
    style::select_by_kind::SelectByKind,
    style::reverse_find::ReverseFind,
    style::return_nil::ReturnNil,
    style::redundant_string_escape::RedundantStringEscape,
    style::redundant_self_assignment_branch::RedundantSelfAssignmentBranch,
    style::redundant_regexp_constructor::RedundantRegexpConstructor,
    style::redundant_regexp_argument::RedundantRegexpArgument,
    style::redundant_min_max_by::RedundantMinMaxBy,
    style::redundant_line_continuation::RedundantLineContinuation,
    style::redundant_interpolation_unfreeze::RedundantInterpolationUnfreeze,
    style::redundant_initialize::RedundantInitialize,
    style::redundant_heredoc_delimiter_quotes::RedundantHeredocDelimiterQuotes,
    style::redundant_format::RedundantFormat,
    style::redundant_filter_chain::RedundantFilterChain,
    style::redundant_each::RedundantEach,
    style::redundant_double_splat_hash_braces::RedundantDoubleSplatHashBraces,
    style::redundant_current_directory_in_path::RedundantCurrentDirectoryInPath,
    style::redundant_constant_base::RedundantConstantBase,
    style::redundant_array_constructor::RedundantArrayConstructor,
    style::redundant_argument::RedundantArgument,
    style::reduce_to_hash::ReduceToHash,
    style::quoted_symbols::QuotedSymbols,
    style::predicate_with_kind::PredicateWithKind,
    style::partition_instead_of_double_select::PartitionInsteadOfDoubleSelect,
    style::operator_method_call::OperatorMethodCall,
    style::open_struct_use::OpenStructUse,
    style::one_class_per_file::OneClassPerFile,
    style::object_then::ObjectThen,
    style::numbered_parameters_limit::NumberedParametersLimit,
    style::numbered_parameters::NumberedParameters,
    style::nil_lambda::NilLambda,
    style::negative_array_index::NegativeArrayIndex,
    style::negated_if_else_condition::NegatedIfElseCondition,
    style::multiline_method_signature::MultilineMethodSignature,
    style::multiline_in_pattern_then::MultilineInPatternThen,
    style::module_member_existence_check::ModuleMemberExistenceCheck,
    style::min_max_comparison::MinMaxComparison,
    style::method_call_with_args_parentheses::MethodCallWithArgsParentheses,
    style::map_to_set::MapToSet,
    style::map_to_hash::MapToHash,
    style::map_join::MapJoin,
    style::map_into_array::MapIntoArray,
    style::map_compact_with_conditional_block::MapCompactWithConditionalBlock,
    style::magic_comment_format::MagicCommentFormat,
    style::keyword_arguments_merging::KeywordArgumentsMerging,
    style::it_assignment::ItAssignment,
    style::in_pattern_then::InPatternThen,
    style::if_with_boolean_literal_branches::IfWithBooleanLiteralBranches,
    style::hash_slice::HashSlice,
    style::hash_fetch_chain::HashFetchChain,
    style::hash_conversion::HashConversion,
    style::file_write::FileWrite,
    style::file_touch::FileTouch,
    style::file_read::FileRead,
    style::file_open::FileOpen,
    style::file_null::FileNull,
    style::file_empty::FileEmpty,
    style::fetch_env_var::FetchEnvVar,
    style::exact_regexp_match::ExactRegexpMatch,
    style::env_home::EnvHome,
    style::endless_method::EndlessMethod,
    style::empty_string_inside_interpolation::EmptyStringInsideInterpolation,
    style::empty_heredoc::EmptyHeredoc,
    style::empty_class_definition::EmptyClassDefinition,
    style::document_dynamic_eval_definition::DocumentDynamicEvalDefinition,
    style::directive_scope::DirectiveScope,
    style::dir_empty::DirEmpty,
    style::dig_chain::DigChain,
    style::date_time::DateTime,
    style::data_inheritance::DataInheritance,
    style::comparable_clamp::ComparableClamp,
    style::comparable_between::ComparableBetween,
    style::combinable_defined::CombinableDefined,
    style::collection_querying::CollectionQuerying,
    style::collection_methods::CollectionMethods,
    style::collection_compact::CollectionCompact,
    style::bitwise_predicate::BitwisePredicate,
    style::auto_resource_cleanup::AutoResourceCleanup,
    style::array_intersect_with_single_element::ArrayIntersectWithSingleElement,
    style::ambiguous_endless_method_definition::AmbiguousEndlessMethodDefinition,
    security::io_methods::IoMethods,
    security::compound_hash::CompoundHash,
    lint::useless_ruby2_keywords::UselessRuby2Keywords,
    lint::useless_rescue::UselessRescue,
    lint::useless_or::UselessOr,
    lint::useless_numeric_operation::UselessNumericOperation,
    lint::useless_defined::UselessDefined,
    lint::useless_default_value_argument::UselessDefaultValueArgument,
    lint::unreachable_pattern_branch::UnreachablePatternBranch,
    lint::unmodified_reduce_accumulator::UnmodifiedReduceAccumulator,
    lint::unexpected_block_arity::UnexpectedBlockArity,
    lint::unescaped_bracket_in_regexp::UnescapedBracketInRegexp,
    lint::triple_quotes::TripleQuotes,
    lint::to_enum_arguments::ToEnumArguments,
    lint::symbol_conversion::SymbolConversion,
    lint::suppressed_exception_in_number_conversion::SuppressedExceptionInNumberConversion,
    lint::super_argument_mismatch::SuperArgumentMismatch,
    lint::shared_mutable_default::SharedMutableDefault,
    lint::require_relative_self_path::RequireRelativeSelfPath,
    lint::require_range_parentheses::RequireRangeParentheses,
    lint::refinement_import_methods::RefinementImportMethods,
    lint::redundant_type_conversion::RedundantTypeConversion,
    lint::redundant_regexp_quantifiers::RedundantRegexpQuantifiers,
    lint::or_assignment_to_constant::OrAssignmentToConstant,
    lint::numeric_operation_with_constant_result::NumericOperationWithConstantResult,
    lint::numbered_parameter_assignment::NumberedParameterAssignment,
    lint::non_atomic_file_operation::NonAtomicFileOperation,
    lint::no_return_in_begin_end_blocks::NoReturnInBeginEndBlocks,
    lint::name_typo::NameTypo,
    lint::mixed_case_range::MixedCaseRange,
    lint::misplaced_magic_comment::MisplacedMagicComment,
    lint::literal_assignment_in_condition::LiteralAssignmentInCondition,
    lint::lambda_without_literal_block::LambdaWithoutLiteralBlock,
    lint::it_without_arguments_in_block::ItWithoutArgumentsInBlock,
    lint::incompatible_io_select_with_fiber_scheduler::IncompatibleIoSelectWithFiberScheduler,
    lint::hash_new_with_keyword_arguments_as_default::HashNewWithKeywordArgumentsAsDefault,
    lint::empty_in_pattern::EmptyInPattern,
    lint::empty_class::EmptyClass,
    lint::duplicate_set_element::DuplicateSetElement,
    lint::duplicate_regexp_character_class_element::DuplicateRegexpCharacterClassElement,
    lint::duplicate_match_pattern::DuplicateMatchPattern,
    lint::duplicate_magic_comment::DuplicateMagicComment,
    lint::deprecated_reference::DeprecatedReference,
    lint::deprecated_constants::DeprecatedConstants,
    lint::data_define_override::DataDefineOverride,
    lint::constant_reassignment::ConstantReassignment,
    lint::constant_overwritten_in_rescue::ConstantOverwrittenInRescue,
    lint::array_literal_in_regexp::ArrayLiteralInRegexp,
    lint::argument_mismatch::ArgumentMismatch,
    lint::ambiguous_range::AmbiguousRange,
    lint::ambiguous_operator_precedence::AmbiguousOperatorPrecedence,
    lint::ambiguous_assignment::AmbiguousAssignment,
    layout::space_before_brackets::SpaceBeforeBrackets,
    layout::multiline_hash_key_line_breaks::MultilineHashKeyLineBreaks,
    layout::line_end_string_concatenation_indentation::LineEndStringConcatenationIndentation,
    layout::line_continuation_spacing::LineContinuationSpacing,
    layout::first_method_parameter_line_break::FirstMethodParameterLineBreak,
    layout::first_method_argument_line_break::FirstMethodArgumentLineBreak,
    layout::first_hash_element_line_break::FirstHashElementLineBreak,
    layout::first_array_element_line_break::FirstArrayElementLineBreak,
    layout::empty_lines_after_module_inclusion::EmptyLinesAfterModuleInclusion,
    layout::class_structure::ClassStructure,
    gemspec::require_mfa::RequireMFA,
    gemspec::development_dependencies::DevelopmentDependencies,
    gemspec::deprecated_attribute_assignment::DeprecatedAttributeAssignment,
    gemspec::attribute_assignment::AttributeAssignment,
    gemspec::add_runtime_dependency::AddRuntimeDependency,
    lint::cop_directive_syntax::CopDirectiveSyntax,
    style::conditional_assignment::ConditionalAssignment,
    metrics::perceived_complexity::PerceivedComplexity,
    metrics::parameter_lists::ParameterLists,
    metrics::module_length::ModuleLength,
    metrics::method_length::MethodLength,
    metrics::cyclomatic_complexity::CyclomaticComplexity,
    metrics::class_length::ClassLength,
    metrics::block_length::BlockLength,
    metrics::abc_size::AbcSize,
    layout::space_inside_range_literal::SpaceInsideRangeLiteral,
    layout::space_inside_percent_literal_delimiters::SpaceInsidePercentLiteralDelimiters,
    layout::space_inside_array_percent_literal::SpaceInsideArrayPercentLiteral,
    layout::space_in_lambda_literal::SpaceInLambdaLiteral,
    layout::space_before_semicolon::SpaceBeforeSemicolon,
    layout::space_before_first_arg::SpaceBeforeFirstArg,
    layout::space_before_comma::SpaceBeforeComma,
    layout::space_before_block_braces::SpaceBeforeBlockBraces,
    layout::space_around_method_call_operator::SpaceAroundMethodCallOperator,
    layout::space_around_block_parameters::SpaceAroundBlockParameters,
    layout::space_after_semicolon::SpaceAfterSemicolon,
    layout::space_after_not::SpaceAfterNot,
    layout::space_after_method_name::SpaceAfterMethodName,
    layout::space_after_comma::SpaceAfterComma,
    layout::space_after_colon::SpaceAfterColon,
    layout::parameter_alignment::ParameterAlignment,
    layout::multiline_operation_indentation::MultilineOperationIndentation,
    layout::multiline_method_definition_brace_layout::MultilineMethodDefinitionBraceLayout,
    layout::multiline_method_call_indentation::MultilineMethodCallIndentation,
    layout::multiline_method_call_brace_layout::MultilineMethodCallBraceLayout,
    layout::multiline_hash_brace_layout::MultilineHashBraceLayout,
    layout::multiline_block_layout::MultilineBlockLayout,
    layout::multiline_array_brace_layout::MultilineArrayBraceLayout,
    layout::leading_comment_space::LeadingCommentSpace,
    layout::first_parameter_indentation::FirstParameterIndentation,
    layout::first_array_element_indentation::FirstArrayElementIndentation,
    layout::end_alignment::EndAlignment,
    layout::empty_lines_around_module_body::EmptyLinesAroundModuleBody,
    layout::empty_lines_around_method_body::EmptyLinesAroundMethodBody,
    layout::empty_lines_around_exception_handling_keywords::EmptyLinesAroundExceptionHandlingKeywords,
    layout::empty_lines_around_block_body::EmptyLinesAroundBlockBody,
    layout::empty_lines_around_begin_body::EmptyLinesAroundBeginBody,
    layout::empty_lines_around_attribute_accessor::EmptyLinesAroundAttributeAccessor,
    layout::empty_lines_around_arguments::EmptyLinesAroundArguments,
    layout::empty_lines_around_access_modifier::EmptyLinesAroundAccessModifier,
    layout::empty_line_after_guard_clause::EmptyLineAfterGuardClause,
    layout::else_alignment::ElseAlignment,
    layout::dot_position::DotPosition,
    layout::def_end_alignment::DefEndAlignment,
    layout::condition_position::ConditionPosition,
    layout::closing_parenthesis_indentation::ClosingParenthesisIndentation,
    layout::closing_heredoc_indentation::ClosingHeredocIndentation,
    layout::case_indentation::CaseIndentation,
    layout::block_end_newline::BlockEndNewline,
    layout::block_alignment::BlockAlignment,
    layout::begin_end_alignment::BeginEndAlignment,
    layout::assignment_indentation::AssignmentIndentation,
    layout::array_alignment::ArrayAlignment,
    layout::access_modifier_indentation::AccessModifierIndentation,
    style::zero_length_predicate::ZeroLengthPredicate,
    style::yoda_condition::YodaCondition,
    style::trivial_accessors::TrivialAccessors,
    style::trailing_underscore_variable::TrailingUnderscoreVariable,
    style::signal_exception::SignalException,
    style::semicolon::Semicolon,
    style::sample::Sample,
    style::safe_navigation::SafeNavigation,
    style::rescue_standard_error::RescueStandardError,
    style::regexp_literal::RegexpLiteral,
    style::redundant_sort::RedundantSort,
    style::redundant_fetch_block::RedundantFetchBlock,
    style::redundant_assignment::RedundantAssignment,
    style::random_with_offset::RandomWithOffset,
    style::raise_args::RaiseArgs,
    style::perl_backrefs::PerlBackrefs,
    style::percent_literal_delimiters::PercentLiteralDelimiters,
    style::parentheses_around_condition::ParenthesesAroundCondition,
    style::parallel_assignment::ParallelAssignment,
    style::one_line_conditional::OneLineConditional,
    style::non_nil_check::NonNilCheck,
    style::next::Next,
    style::multiple_comparison::MultipleComparison,
    style::module_function::ModuleFunction,
    style::mixin_grouping::MixinGrouping,
    style::method_def_parentheses::MethodDefParentheses,
    style::method_call_without_args_parentheses::MethodCallWithoutArgsParentheses,
    style::lambda::Lambda,
    style::inverse_methods::InverseMethods,
    style::infinite_loop::InfiniteLoop,
    style::if_with_semicolon::IfWithSemicolon,
    style::if_inside_else::IfInsideElse,
    style::identical_conditional_branches::IdenticalConditionalBranches,
    style::hash_each_methods::HashEachMethods,
    style::format_string_token::FormatStringToken,
    style::format_string::FormatString,
    style::float_division::FloatDivision,
    style::explicit_block_argument::ExplicitBlockArgument,
    style::expand_path_arguments::ExpandPathArguments,
    style::eval_with_location::EvalWithLocation,
    style::empty_literal::EmptyLiteral,
    style::empty_case_condition::EmptyCaseCondition,
    style::each_with_object::EachWithObject,
    style::double_negation::DoubleNegation,
    style::command_literal::CommandLiteral,
    style::combinable_loops::CombinableLoops,
    style::class_equality_comparison::ClassEqualityComparison,
    style::case_like_if::CaseLikeIf,
    style::block_delimiters::BlockDelimiters,
    style::and_or::AndOr,
    style::alias::Alias,
    style::empty_method::EmptyMethod,
    style::rescue_modifier::RescueModifier,
    style::redundant_percent_q::RedundantPercentQ,
    style::redundant_self_assignment::RedundantSelfAssignment,
    style::case_equality::CaseEquality,
    style::nested_modifier::NestedModifier,
    style::hash_as_last_array_item::HashAsLastArrayItem,
    style::negated_if::NegatedIf,
    style::multiline_memoization::MultilineMemoization,
    style::self_assignment::SelfAssignment,
    style::or_assignment::OrAssignment,
    style::r#for::For,
    style::negated_unless::NegatedUnless,
    style::nil_comparison::NilComparison,
    style::each_for_simple_loop::EachForSimpleLoop,
    style::redundant_exception::RedundantException,
    style::redundant_conditional::RedundantConditional,
    style::keyword_parameters_order::KeywordParametersOrder,
    style::attr::Attr,
    style::struct_inheritance::StructInheritance,
    style::stabby_lambda_parentheses::StabbyLambdaParentheses,
    style::redundant_sort_by::RedundantSortBy,
    style::nested_parenthesized_calls::NestedParenthesizedCalls,
    style::lambda_call::LambdaCall,
    style::global_std_stream::GlobalStdStream,
    style::global_vars::GlobalVars,
    style::string_literals_in_interpolation::StringLiteralsInInterpolation,
    style::not::Not,
    style::bare_percent_literals::BarePercentLiterals,
    style::preferred_hash_methods::PreferredHashMethods,
    style::single_argument_dig::SingleArgumentDig,
    style::min_max::MinMax,
    style::nested_ternary_operator::NestedTernaryOperator,
    style::trailing_method_end_statement::TrailingMethodEndStatement,
    style::redundant_file_extension_in_require::RedundantFileExtensionInRequire,
    style::multiline_when_then::MultilineWhenThen,
    style::multiline_if_modifier::MultilineIfModifier,
    style::stderr_puts::StderrPuts,
    style::character_literal::CharacterLiteral,
    style::unless_else::UnlessElse,
    style::even_odd::EvenOdd,
    style::trailing_body_on_method_definition::TrailingBodyOnMethodDefinition,
    style::class_check::ClassCheck,
    style::class_methods::ClassMethods,
    style::while_until_modifier::WhileUntilModifier,
    style::while_until_do::WhileUntilDo,
    style::empty_block_parameter::EmptyBlockParameter,
    style::redundant_capital_w::RedundantCapitalW,
    style::def_with_parentheses::DefWithParentheses,
    style::colon_method_call::ColonMethodCall,
    style::strip::Strip,
    style::variable_interpolation::VariableInterpolation,
    style::multiline_if_then::MultilineIfThen,
    style::empty_lambda_parameter::EmptyLambdaParameter,
    style::trailing_body_on_class::TrailingBodyOnClass,
    style::trailing_body_on_module::TrailingBodyOnModule,
    style::negated_while::NegatedWhile,
    style::array_join::ArrayJoin,
    style::when_then::WhenThen,
    style::proc::Proc,
    style::colon_method_definition::ColonMethodDefinition,
    style::symbol_literal::SymbolLiteral,
    style::end_block::EndBlock,
    style::percent_q_literals::PercentQLiterals,
    naming::variable_number::VariableNumber,
    naming::rescued_exceptions_variable_name::RescuedExceptionsVariableName,
    naming::predicate_prefix::PredicatePrefix,
    naming::memoized_instance_variable_name::MemoizedInstanceVariableName,
    lint::void::Void,
    lint::useless_setter_call::UselessSetterCall,
    lint::unused_method_argument::UnusedMethodArgument,
    lint::unused_block_argument::UnusedBlockArgument,
    lint::underscore_prefixed_variable_name::UnderscorePrefixedVariableName,
    lint::shadowed_argument::ShadowedArgument,
    lint::safe_navigation_consistency::SafeNavigationConsistency,
    lint::redundant_splat_expansion::RedundantSplatExpansion,
    lint::redundant_safe_navigation::RedundantSafeNavigation,
    lint::literal_in_interpolation::LiteralInInterpolation,
    lint::literal_as_condition::LiteralAsCondition,
    lint::ineffective_access_modifier::IneffectiveAccessModifier,
    lint::implicit_string_concatenation::ImplicitStringConcatenation,
    lint::deprecated_open_ssl_constant::DeprecatedOpenSSLConstant,
    lint::assignment_in_condition::AssignmentInCondition,
    security::yaml_load::YAMLLoad,
    security::json_load::JSONLoad,
    naming::heredoc_delimiter_naming::HeredocDelimiterNaming,
    naming::heredoc_delimiter_case::HeredocDelimiterCase,
    naming::class_and_module_camel_case::ClassAndModuleCamelCase,
    naming::binary_operator_parameter_name::BinaryOperatorParameterName,
    lint::suppressed_exception::SuppressedException,
    lint::deprecated_class_methods::DeprecatedClassMethods,
    lint::redundant_require_statement::RedundantRequireStatement,
    lint::raise_exception::RaiseException,
    lint::inherit_exception::InheritException,
    lint::disjunctive_assignment_in_constructor::DisjunctiveAssignmentInConstructor,
    lint::send_with_mixin_argument::SendWithMixinArgument,
    lint::useless_times::UselessTimes,
    lint::useless_method_definition::UselessMethodDefinition,
    lint::trailing_comma_in_attribute_declaration::TrailingCommaInAttributeDeclaration,
    lint::top_level_return_with_argument::TopLevelReturnWithArgument,
    lint::to_json::ToJSON,
    lint::safe_navigation_with_empty::SafeNavigationWithEmpty,
    lint::rescue_type::RescueType,
    lint::regexp_as_condition::RegexpAsCondition,
    lint::redundant_with_object::RedundantWithObject,
    lint::redundant_with_index::RedundantWithIndex,
    lint::percent_symbol_array::PercentSymbolArray,
    lint::percent_string_array::PercentStringArray,
    lint::parentheses_as_grouped_expression::ParenthesesAsGroupedExpression,
    lint::multiple_comparison::MultipleComparison,
    lint::r#loop::Loop,
    lint::interpolation_check::InterpolationCheck,
    lint::identity_comparison::IdentityComparison,
    lint::empty_interpolation::EmptyInterpolation,
    lint::duplicate_require::DuplicateRequire,
    lint::boolean_symbol::BooleanSymbol,
    lint::big_decimal_new::BigDecimalNew,
    bundler::insecure_protocol_source::InsecureProtocolSource,
    lint::empty_ensure::EmptyEnsure,
    naming::file_name::FileName,
    naming::ascii_identifiers::AsciiIdentifiers,
    migration::department_name::DepartmentName,
    metrics::block_nesting::BlockNesting,
    lint::redundant_cop_enable_directive::RedundantCopEnableDirective,
    lint::missing_cop_enable_directive::MissingCopEnableDirective,
    gemspec::required_ruby_version::RequiredRubyVersion,
    gemspec::ordered_dependencies::OrderedDependencies,
    gemspec::duplicated_assignment::DuplicatedAssignment,
    bundler::ordered_gems::OrderedGems,
    bundler::duplicated_group::DuplicatedGroup,
    bundler::duplicated_gem::DuplicatedGem,
    layout::space_around_keyword::SpaceAroundKeyword,
    layout::rescue_ensure_alignment::RescueEnsureAlignment,
    lint::safe_navigation_chain::SafeNavigationChain,
    lint::useless_else_without_rescue::UselessElseWithoutRescue,
    lint::script_permission::ScriptPermission,
    lint::ordered_magic_comments::OrderedMagicComments,
    lint::non_deterministic_require_order::NonDeterministicRequireOrder,
    style::redundant_interpolation::RedundantInterpolation,
    style::redundant_begin::RedundantBegin,
    style::disable_cops_within_source_code_directive::DisableCopsWithinSourceCodeDirective,
    style::bisected_attr_accessor::BisectedAttrAccessor,
    style::access_modifier_declarations::AccessModifierDeclarations,
    style::single_line_methods::SingleLineMethods,
    style::encoding::Encoding,
    style::commented_keyword::CommentedKeyword,
    style::comment_annotation::CommentAnnotation,
    style::block_comments::BlockComments,
    style::hash_transform_values::HashTransformValues,
    style::hash_transform_keys::HashTransformKeys,
    style::line_end_concatenation::LineEndConcatenation,
    style::numeric_predicate::NumericPredicate,
    style::ternary_parentheses::TernaryParentheses,
    style::symbol_array::SymbolArray,
    style::special_global_vars::SpecialGlobalVars,
    style::multiline_ternary_operator::MultilineTernaryOperator,
    style::redundant_freeze::RedundantFreeze,
    style::slicing_with_range::SlicingWithRange,
    style::unpack_first::UnpackFirst,
    style::dir::Dir,
    lint::ambiguous_operator::AmbiguousOperator,
    lint::ambiguous_regexp_literal::AmbiguousRegexpLiteral,
    lint::out_of_range_regexp_ref::OutOfRangeRegexpRef,
    lint::uri_regexp::UriRegexp,
    lint::unified_integer::UnifiedInteger,
    lint::empty_conditional_body::EmptyConditionalBody,
    lint::empty_when::EmptyWhen,
    bundler::gem_filename::GemFilename,
    layout::heredoc_indentation::HeredocIndentation,
    lint::redundant_cop_disable_directive::RedundantCopDisableDirective,
    lint::binary_operator_with_identical_operands::BinaryOperatorWithIdenticalOperands,
    lint::circular_argument_reference::CircularArgumentReference,
    style::class_and_module_children::ClassAndModuleChildren,
    style::empty_else::EmptyElse,
    style::string_concatenation::StringConcatenation,
    lint::debugger::Debugger,
    lint::duplicate_case_condition::DuplicateCaseCondition,
    lint::duplicate_elsif_condition::DuplicateElsifCondition,
    lint::duplicate_hash_key::DuplicateHashKey,
    lint::duplicate_methods::DuplicateMethods,
    lint::duplicate_rescue_exception::DuplicateRescueException,
    style::redundant_regexp_character_class::RedundantRegexpCharacterClass,
    style::redundant_regexp_escape::RedundantRegexpEscape,
    style::numeric_literal_prefix::NumericLiteralPrefix,
    style::if_unless_modifier_of_if_unless::IfUnlessModifierOfIfUnless,
    lint::ambiguous_block_association::AmbiguousBlockAssociation,
    lint::else_layout::ElseLayout,
    lint::redundant_string_coercion::RedundantStringCoercion,
    lint::empty_block::EmptyBlock,
    lint::empty_expression::EmptyExpression,
    lint::empty_file::EmptyFile,
    lint::erb_new_arguments::ErbNewArguments,
    lint::ensure_return::EnsureReturn,
    lint::each_with_object_argument::EachWithObjectArgument,
    layout::empty_line_between_defs::EmptyLineBetweenDefs,
    layout::empty_lines_around_class_body::EmptyLinesAroundClassBody,
    layout::space_around_operators::SpaceAroundOperators,
    layout::space_inside_hash_literal_braces::SpaceInsideHashLiteralBraces,
    layout::space_inside_parens::SpaceInsideParens,
    layout::space_inside_string_interpolation::SpaceInsideStringInterpolation,
    style::symbol_proc::SymbolProc,
    layout::space_inside_array_literal_brackets::SpaceInsideArrayLiteralBrackets,
    layout::space_inside_block_braces::SpaceInsideBlockBraces,
    layout::space_inside_reference_brackets::SpaceInsideReferenceBrackets,
    layout::extra_spacing::ExtraSpacing,
    layout::hash_alignment::HashAlignment,
    style::word_array::WordArray,
    style::numeric_literals::NumericLiterals,
    style::redundant_parentheses::RedundantParentheses,
    style::redundant_condition::RedundantCondition,
    layout::first_hash_element_indentation::FirstHashElementIndentation,
    layout::first_argument_indentation::FirstArgumentIndentation,
    layout::argument_alignment::ArgumentAlignment,
    style::redundant_return::RedundantReturn,
    style::sole_nested_conditional::SoleNestedConditional,
    layout::line_length::LineLength,
    style::frozen_string_literal_comment::FrozenStringLiteralComment,
    style::string_literals::StringLiterals,
    layout::trailing_empty_lines::TrailingEmptyLines,
    layout::indentation_width::IndentationWidth,
    layout::empty_lines::EmptyLines,
    layout::indentation_consistency::IndentationConsistency,
    style::documentation::Documentation,
    style::guard_clause::GuardClause,
    style::if_unless_modifier::IfUnlessModifier,
    style::hash_syntax::HashSyntax,
    style::mutable_constant::MutableConstant,
    style::trailing_comma_in_arguments::TrailingCommaInArguments,
    style::trailing_comma_in_hash_literal::TrailingCommaInHashLiteral,
    style::trailing_comma_in_array_literal::TrailingCommaInArrayLiteral,
    style::accessor_grouping::AccessorGrouping,
    layout::trailing_whitespace::TrailingWhitespace,
    layout::leading_empty_lines::LeadingEmptyLines,
    layout::space_before_comment::SpaceBeforeComment,
    style::optional_boolean_parameter::OptionalBooleanParameter,
    lint::shadowed_exception::ShadowedException,
    lint::missing_super::MissingSuper,
    lint::constant_resolution::ConstantResolution,
    lint::useless_access_modifier::UselessAccessModifier,
    style::redundant_self::RedundantSelf,
    lint::number_conversion::NumberConversion,
    lint::self_assignment::SelfAssignment,
    lint::useless_assignment::UselessAssignment,
    lint::shadowing_outer_local_variable::ShadowingOuterLocalVariable,
    lint::float_out_of_range::FloatOutOfRange,
    lint::flip_flop::FlipFlop,
    lint::hash_compare_by_identity::HashCompareByIdentity,
    lint::constant_definition_in_block::ConstantDefinitionInBlock,
    lint::mixed_regexp_capture_types::MixedRegexpCaptureTypes,
    lint::float_comparison::FloatComparison,
    lint::uri_escape_unescape::UriEscapeUnescape,
    security::marshal_load::MarshalLoad,
    security::eval::Eval,
    security::open::Open,
    naming::accessor_method_name::AccessorMethodName,
    naming::constant_name::ConstantName,
    style::exponential_notation::ExponentialNotation,
    style::optional_arguments::OptionalArguments,
    lint::rescue_exception::RescueException,
    style::begin_block::BeginBlock,
    lint::return_in_void_context::ReturnInVoidContext,
    lint::struct_new_override::StructNewOverride,
    style::class_vars::ClassVars,
    lint::rand_one::RandOne,
    style::multiline_block_chain::MultilineBlockChain,
    lint::next_without_accumulator::NextWithoutAccumulator,
    lint::nested_percent_literal::NestedPercentLiteral,
    lint::nested_method_definition::NestedMethodDefinition,
    lint::require_parentheses::RequireParentheses,
    lint::non_local_exit_from_iterator::NonLocalExitFromIterator,
    gemspec::ruby_version_globals_usage::RubyVersionGlobalsUsage,
    style::mixin_usage::MixinUsage,
    style::hash_like_case::HashLikeCase,
    style::missing_respond_to_missing::MissingRespondToMissing,
    lint::unreachable_loop::UnreachableLoop,
    lint::format_parameter_mismatch::FormatParameterMismatch,
    naming::block_parameter_name::BlockParameterName,
    naming::method_parameter_name::MethodParameterName,
    naming::method_name::MethodName,
    naming::variable_name::VariableName,
    lint::unreachable_code::UnreachableCode,
    layout::space_around_equals_in_parameter_default::SpaceAroundEqualsInParameterDefault,
    layout::empty_comment::EmptyComment,
    layout::end_of_line::EndOfLine,
    layout::comment_indentation::CommentIndentation,
    layout::indentation_style::IndentationStyle,
    layout::empty_line_after_magic_comment::EmptyLineAfterMagicComment,
    layout::initial_indentation::InitialIndentation,
}

/// Per-rule constants derived from [`Rule::META`] at compile time.
trait RuleExt: Rule {
    /// `META` as a `'static` reference, for [`ALL_RULES`] and [`RuleOptions`].
    const META_REF: &'static RuleMeta = &Self::META;
    /// `true` at `kind as usize` for every node kind the rule subscribed to.
    const SUBSCRIBED: [bool; NodeKind::COUNT] = subscription_table(Self::META.kinds);
}

impl<R: Rule> RuleExt for R {}

/// Converts one configuration value to the rule-facing [`OptionValue`].
pub fn option_value(value: &YamlValue) -> OptionValue {
    match value {
        YamlValue::Null => OptionValue::Null,
        YamlValue::Bool(b) => OptionValue::Bool(*b),
        YamlValue::Int(i) => OptionValue::Int(*i),
        YamlValue::Float(f) => OptionValue::Float(*f),
        YamlValue::String(s) | YamlValue::Regexp(s) => OptionValue::Str(s.clone()),
        YamlValue::Array(items) => OptionValue::List(items.iter().map(option_value).collect()),
        YamlValue::Mapping(map) => OptionValue::Map(
            map.iter().map(|(key, value)| (key.to_string(), option_value(value))).collect(),
        ),
    }
}

/// A cop's configured options, in the rule-facing representation, plus its
/// resolved `Enabled` flag so a rule can mirror RuboCop's `for_enabled_cop`
/// (a disabled peer's options do not apply). The raw `Enabled` value is
/// dropped from the parameter chain -- it would otherwise appear a second
/// time under the same key, shadowed by the resolved flag -- and its one
/// piece of information the flag cannot carry, `Enabled: pending`, is
/// surfaced as `EnabledPending` instead (RuboCop's `Config#for_cop`
/// reporting `'pending'`, which `Lint/RedundantCopDisableDirective`'s
/// `pending_cop_not_run?` needs).
fn cop_options(cop: &CopConfig) -> Vec<(String, OptionValue)> {
    std::iter::once(("Enabled".to_string(), OptionValue::Bool(cop.enabled)))
        .chain(cop.is_pending().then(|| ("EnabledPending".to_string(), OptionValue::Bool(true))))
        .chain(
            cop.options
                .iter()
                .filter(|(key, _)| key.as_str() != "Enabled")
                .map(|(key, value)| (key.clone(), option_value(value))),
        )
        .collect()
}

/// Every cop's options plus `AllCops`, so a rule can read another cop's
/// settings or global ones such as `TargetRubyVersion`.
fn peer_options(cfg: &LoadedConfig) -> PeerOptions {
    let mut peers: PeerOptions =
        cfg.cops().map(|(name, cop)| (name.to_string(), cop_options(cop))).collect();
    let all_cops = cfg
        .all_cops()
        .raw()
        .iter()
        .map(|(key, value)| (key.to_string(), option_value(value)))
        .collect();
    peers.insert("AllCops".to_string(), all_cops);
    peers
}

/// Everything the slot list needs to decide whether a rule runs and with
/// which options.
struct Builder<'a> {
    cfg: Option<&'a LoadedConfig>,
    /// When set, exactly these cops run, regardless of `Enabled`, mirroring
    /// RuboCop's `--only`.
    only: Option<&'a [&'a str]>,
    /// Whether `only` came from the CLI's `--only` (see [`RuleOptions::only_run`]).
    only_run: bool,
    peers: Arc<PeerOptions>,
}

impl Builder<'_> {
    fn configure<R: Rule>(&self) -> Result<Option<R>, OptionError> {
        let meta = <R as RuleExt>::META_REF;
        let cop = self.cfg.and_then(|cfg| cfg.cop(meta.name));
        let enabled = match self.only {
            Some(names) => names.contains(&meta.name),
            None => cop.map_or(meta.enabled_by_default, |cop| cop.enabled),
        };
        if !enabled {
            return Ok(None);
        }
        let own = cop.map(cop_options).unwrap_or_default();
        let options =
            RuleOptions::new(meta, own, Arc::clone(&self.peers)).with_only_run(self.only_run);
        R::configure(&options).map(Some)
    }
}

/// One slot list: a tree of tuples whose leaves are optional rules.
trait SlotList: Clone + Send + Sync + 'static + Sized {
    fn build(builder: &Builder<'_>) -> Result<Self, OptionError>;
    fn add_interest(&self, interest: &mut [bool; NodeKind::COUNT]);
    fn file_start(&mut self, ctx: &mut Context<'_>);
    fn enter(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>);
    fn leave(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>);
    fn file_end(&mut self, ctx: &mut Context<'_>);
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]);
}

impl<R: Rule> SlotList for Option<R> {
    fn build(builder: &Builder<'_>) -> Result<Self, OptionError> {
        builder.configure::<R>()
    }

    fn add_interest(&self, interest: &mut [bool; NodeKind::COUNT]) {
        if self.is_some() {
            for (slot, subscribed) in interest.iter_mut().zip(<R as RuleExt>::SUBSCRIBED) {
                *slot |= subscribed;
            }
        }
    }

    #[inline]
    fn file_start(&mut self, ctx: &mut Context<'_>) {
        if let Some(rule) = self {
            rule.file_start(ctx);
        }
    }

    #[inline]
    fn enter(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(rule) = self {
            if <R as RuleExt>::SUBSCRIBED[kind as usize] {
                rule.enter(node, ctx);
            }
        }
    }

    #[inline]
    fn leave(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(rule) = self {
            if <R as RuleExt>::SUBSCRIBED[kind as usize] {
                rule.leave(node, ctx);
            }
        }
    }

    #[inline]
    fn file_end(&mut self, ctx: &mut Context<'_>) {
        if let Some(rule) = self {
            rule.file_end(ctx);
        }
    }

    #[inline]
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]) {
        if let Some(rule) = self {
            rule.file_finish(ctx, reported);
        }
    }
}

impl<A: SlotList, B: SlotList> SlotList for (A, B) {
    fn build(builder: &Builder<'_>) -> Result<Self, OptionError> {
        Ok((A::build(builder)?, B::build(builder)?))
    }

    fn add_interest(&self, interest: &mut [bool; NodeKind::COUNT]) {
        self.0.add_interest(interest);
        self.1.add_interest(interest);
    }

    #[inline]
    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.0.file_start(ctx);
        self.1.file_start(ctx);
    }

    #[inline]
    fn enter(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        self.0.enter(kind, node, ctx);
        self.1.enter(kind, node, ctx);
    }

    #[inline]
    fn leave(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        self.0.leave(kind, node, ctx);
        self.1.leave(kind, node, ctx);
    }

    #[inline]
    fn file_end(&mut self, ctx: &mut Context<'_>) {
        self.0.file_end(ctx);
        self.1.file_end(ctx);
    }

    #[inline]
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]) {
        self.0.file_finish(ctx, reported);
        self.1.file_finish(ctx, reported);
    }
}

impl<T: SlotList> SlotList for Box<T> {
    fn build(builder: &Builder<'_>) -> Result<Self, OptionError> {
        T::build(builder).map(Box::new)
    }

    fn add_interest(&self, interest: &mut [bool; NodeKind::COUNT]) {
        (**self).add_interest(interest);
    }

    #[inline]
    fn file_start(&mut self, ctx: &mut Context<'_>) {
        (**self).file_start(ctx);
    }

    #[inline]
    fn enter(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        (**self).enter(kind, node, ctx);
    }

    #[inline]
    fn leave(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        (**self).leave(kind, node, ctx);
    }

    #[inline]
    fn file_end(&mut self, ctx: &mut Context<'_>) {
        (**self).file_end(ctx);
    }

    #[inline]
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]) {
        (**self).file_finish(ctx, reported);
    }
}

/// The configured set of enabled rules for one effective configuration.
///
/// Cloned per file: rules keep per-file state in `self`.
#[derive(Clone)]
pub struct RuleSet {
    slots: Slots,
    /// `true` at `kind as usize` when any enabled rule subscribed to it.
    interest: [bool; NodeKind::COUNT],
}

impl RuleSet {
    fn from_builder(builder: &Builder<'_>) -> Result<Self, OptionError> {
        let slots = <Slots as SlotList>::build(builder)?;
        let mut interest = [false; NodeKind::COUNT];
        slots.add_interest(&mut interest);
        Ok(Self { slots, interest })
    }

    /// Every rule enabled by default, with RuboCop's default options.
    ///
    /// # Panics
    ///
    /// If a rule rejects its own declared defaults, which is a bug in that
    /// rule's schema.
    pub fn rubocop_defaults() -> Self {
        let builder =
            Builder { cfg: None, only: None, only_run: false, peers: Arc::new(PeerOptions::new()) };
        Self::from_builder(&builder)
            .unwrap_or_else(|err| panic!("rule rejected its own defaults: {err}"))
    }

    /// The rules `cfg` enables, configured from it.
    pub fn from_config(cfg: &LoadedConfig) -> Result<Self, OptionError> {
        let builder = Builder {
            cfg: Some(cfg),
            only: None,
            only_run: false,
            peers: Arc::new(peer_options(cfg)),
        };
        Self::from_builder(&builder)
    }

    /// Only `names`, configured from `cfg`, enabled regardless of what
    /// `cfg` says about them (RuboCop's `--only`).
    pub fn only(names: &[&str], cfg: &LoadedConfig) -> Result<Self, OptionError> {
        Self::restricted(names, cfg, true)
    }

    /// Only `names`, configured from `cfg` and enabled regardless of it, the
    /// way RuboCop's `CopHelper` runs a cop in its specs: unlike
    /// [`RuleSet::only`], the run is not an `--only` run, so rules that read
    /// the registry see every configured cop.
    pub fn isolated(names: &[&str], cfg: &LoadedConfig) -> Result<Self, OptionError> {
        Self::restricted(names, cfg, false)
    }

    fn restricted(names: &[&str], cfg: &LoadedConfig, only_run: bool) -> Result<Self, OptionError> {
        let builder = Builder {
            cfg: Some(cfg),
            only: Some(names),
            only_run,
            peers: Arc::new(peer_options(cfg)),
        };
        Self::from_builder(&builder)
    }
}

impl Dispatch for RuleSet {
    #[inline]
    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.slots.file_start(ctx);
    }

    #[inline]
    fn enter(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.interest[kind as usize] {
            return;
        }
        self.slots.enter(kind, node, ctx);
    }

    #[inline]
    fn leave(&mut self, kind: NodeKind, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.interest[kind as usize] {
            return;
        }
        self.slots.leave(kind, node, ctx);
    }

    #[inline]
    fn file_end(&mut self, ctx: &mut Context<'_>) {
        self.slots.file_end(ctx);
    }

    #[inline]
    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]) {
        self.slots.file_finish(ctx, reported);
    }
}
