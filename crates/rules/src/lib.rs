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
/// -- cannot collide. Rules are grouped eight to a balanced subtree before
/// chaining, keeping type nesting (and rustc's drop-check/auto-trait
/// recursion over it) at roughly an eighth of the rule count.
macro_rules! rule_set {
    ($($rule:path),+ $(,)?) => {
        /// Every registered rule's metadata, in registration order.
        pub const ALL_RULES: &[&'static RuleMeta] = &[$(<$rule as RuleExt>::META_REF),+];

        /// The configured rules [`RuleSet`] dispatches to.
        type Slots = rule_set!(@slots $($rule),+);
    };
    (@slots $a:path, $b:path, $c:path, $d:path, $e:path, $f:path, $g:path, $h:path, $($rest:path),+) => {
        (
            (
                ((Option<$a>, Option<$b>), (Option<$c>, Option<$d>)),
                ((Option<$e>, Option<$f>), (Option<$g>, Option<$h>)),
            ),
            rule_set!(@slots $($rest),+),
        )
    };
    (@slots $head:path) => { Option<$head> };
    (@slots $head:path, $($tail:path),+) => { (Option<$head>, rule_set!(@slots $($tail),+)) };
}

rule_set! {
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
/// (a disabled peer's options do not apply).
fn cop_options(cop: &CopConfig) -> Vec<(String, OptionValue)> {
    std::iter::once(("Enabled".to_string(), OptionValue::Bool(cop.enabled)))
        .chain(cop.options.iter().map(|(key, value)| (key.clone(), option_value(value))))
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
