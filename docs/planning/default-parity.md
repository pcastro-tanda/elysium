# Default-cop parity inventory
Generated from RuboCop 1.82.1 `config/default.yml` (`Enabled: true` only; `pending` cops excluded) minus the rules registered in `docs/rules/`. Regenerate with the script in this file's git history / `tools/` once ported there.
**338 default-enabled cops missing of 394** (60 implemented).
## By department
- Style: 146
- Lint: 88
- Layout: 64
- Naming: 16
- Metrics: 9
- Bundler: 5
- Security: 5
- Gemspec: 4
- Migration: 1

## By required infrastructure
Coarse, from mixins/API usage in the cop source. `pure-ast` = only node callbacks + `add_offense`. `tokens/comments` = needs the token stream or comment list (we have directives only). `target-ruby` = branches on `TargetRubyVersion`. `metrics` = needs code-length/complexity utilities. `semantic` = uses `VariableForce`.
- autocorrect: 257
- config-options: 52
- pure-ast: 45
- file-level: 39
- metrics: 30
- tokens/comments: 26
- target-ruby: 25
- semantic: 5
- ?: 4

## Full list
| Cop | src lines | spec `it`s | needs |
|---|---|---|---|
| Bundler/DuplicatedGem | 94 | 10 | file-level |
| Bundler/DuplicatedGroup | 127 | 21 | file-level |
| Bundler/GemFilename | 102 | 2 | file-level |
| Bundler/InsecureProtocolSource | 85 | 6 | autocorrect |
| Bundler/OrderedGems | 69 | 17 | autocorrect, file-level |
| Gemspec/DuplicatedAssignment | 111 | 19 | file-level |
| Gemspec/OrderedDependencies | 100 | 8 | autocorrect, file-level |
| Gemspec/RequiredRubyVersion | 129 | 21 | target-ruby |
| Gemspec/RubyVersionGlobalsUsage | 55 | 5 | pure-ast |
| Layout/AccessModifierIndentation | 104 | 41 | autocorrect |
| Layout/ArrayAlignment | 84 | 25 | autocorrect, config-options |
| Layout/AssignmentIndentation | 57 | 10 | autocorrect |
| Layout/BeginEndAlignment | 73 | 0 | autocorrect |
| Layout/BlockAlignment | 259 | 79 | autocorrect |
| Layout/BlockEndNewline | 83 | 22 | autocorrect |
| Layout/CaseIndentation | 219 | 50 | autocorrect, config-options |
| Layout/ClosingHeredocIndentation | 123 | 11 | autocorrect |
| Layout/ClosingParenthesisIndentation | 193 | 43 | autocorrect |
| Layout/CommentIndentation | 168 | 16 | tokens/comments, autocorrect, file-level, config-options |
| Layout/ConditionPosition | 60 | 5 | autocorrect |
| Layout/DefEndAlignment | 73 | 4 | autocorrect |
| Layout/DotPosition | 139 | 39 | autocorrect |
| Layout/ElseAlignment | 156 | 53 | autocorrect |
| Layout/EmptyComment | 153 | 14 | tokens/comments, autocorrect, file-level, config-options |
| Layout/EmptyLineAfterGuardClause | 217 | 47 | autocorrect, metrics |
| Layout/EmptyLineAfterMagicComment | 71 | 17 | autocorrect, file-level |
| Layout/EmptyLinesAroundAccessModifier | 241 | 48 | autocorrect |
| Layout/EmptyLinesAroundArguments | 82 | 22 | autocorrect |
| Layout/EmptyLinesAroundAttributeAccessor | 139 | 20 | autocorrect |
| Layout/EmptyLinesAroundBeginBody | 41 | 10 | autocorrect |
| Layout/EmptyLinesAroundBlockBody | 41 | 12 | autocorrect |
| Layout/EmptyLinesAroundExceptionHandlingKeywords | 139 | 13 | autocorrect |
| Layout/EmptyLinesAroundMethodBody | 64 | 14 | autocorrect |
| Layout/EmptyLinesAroundModuleBody | 59 | 18 | autocorrect |
| Layout/EndAlignment | 214 | 14 | autocorrect |
| Layout/EndOfLine | 92 | 13 | tokens/comments, file-level |
| Layout/FirstArrayElementIndentation | 189 | 53 | autocorrect |
| Layout/FirstParameterIndentation | 101 | 20 | autocorrect |
| Layout/HeredocIndentation | 162 | 20 | target-ruby, autocorrect |
| Layout/IndentationStyle | 115 | 25 | autocorrect, file-level |
| Layout/InitialIndentation | 55 | 8 | tokens/comments, autocorrect, file-level |
| Layout/LeadingCommentSpace | 203 | 27 | tokens/comments, autocorrect, file-level, metrics, config-options |
| Layout/LeadingEmptyLines | 48 | 10 | tokens/comments, autocorrect, file-level |
| Layout/MultilineArrayBraceLayout | 115 | 4 | autocorrect |
| Layout/MultilineBlockLayout | 164 | 32 | autocorrect |
| Layout/MultilineHashBraceLayout | 115 | 3 | autocorrect |
| Layout/MultilineMethodCallBraceLayout | 132 | 15 | autocorrect |
| Layout/MultilineMethodCallIndentation | 267 | 120 | autocorrect, config-options |
| Layout/MultilineMethodDefinitionBraceLayout | 128 | 3 | autocorrect |
| Layout/MultilineOperationIndentation | 130 | 60 | autocorrect, config-options |
| Layout/ParameterAlignment | 118 | 19 | autocorrect, config-options |
| Layout/RescueEnsureAlignment | 220 | 86 | tokens/comments, autocorrect, file-level |
| Layout/SpaceAfterColon | 49 | 12 | autocorrect |
| Layout/SpaceAfterComma | 32 | 5 | autocorrect |
| Layout/SpaceAfterMethodName | 39 | 8 | autocorrect |
| Layout/SpaceAfterNot | 39 | 6 | autocorrect |
| Layout/SpaceAfterSemicolon | 39 | 7 | autocorrect |
| Layout/SpaceAroundBlockParameters | 162 | 42 | autocorrect |
| Layout/SpaceAroundEqualsInParameterDefault | 89 | 11 | tokens/comments, autocorrect |
| Layout/SpaceAroundKeyword | 274 | 5 | target-ruby, autocorrect |
| Layout/SpaceAroundMethodCallOperator | 98 | 17 | autocorrect |
| Layout/SpaceBeforeBlockBraces | 164 | 20 | autocorrect, config-options |
| Layout/SpaceBeforeComma | 29 | 6 | autocorrect |
| Layout/SpaceBeforeComment | 34 | 5 | autocorrect, file-level |
| Layout/SpaceBeforeFirstArg | 73 | 12 | autocorrect |
| Layout/SpaceBeforeSemicolon | 24 | 8 | autocorrect |
| Layout/SpaceInLambdaLiteral | 78 | 15 | autocorrect |
| Layout/SpaceInsideArrayPercentLiteral | 46 | 9 | autocorrect |
| Layout/SpaceInsideParens | 176 | 28 | autocorrect, file-level |
| Layout/SpaceInsidePercentLiteralDelimiters | 94 | 15 | autocorrect |
| Layout/SpaceInsideRangeLiteral | 54 | 7 | autocorrect |
| Layout/SpaceInsideReferenceBrackets | 143 | 47 | tokens/comments, autocorrect, config-options |
| Layout/SpaceInsideStringInterpolation | 63 | 12 | tokens/comments, autocorrect |
| Lint/AmbiguousOperator | 105 | 17 | autocorrect, file-level |
| Lint/AmbiguousRegexpLiteral | 78 | 15 | target-ruby, autocorrect, file-level |
| Lint/AssignmentInCondition | 107 | 37 | autocorrect |
| Lint/BigDecimalNew | 41 | 3 | autocorrect |
| Lint/BinaryOperatorWithIdenticalOperands | 66 | 5 | pure-ast |
| Lint/BooleanSymbol | 61 | 10 | autocorrect |
| Lint/CircularArgumentReference | 106 | 13 | target-ruby |
| Lint/ConstantDefinitionInBlock | 100 | 29 | pure-ast |
| Lint/DeprecatedClassMethods | 118 | 31 | autocorrect |
| Lint/DeprecatedOpenSSLConstant | 0 | 0 | ? |
| Lint/DisjunctiveAssignmentInConstructor | 110 | 7 | autocorrect |
| Lint/DuplicateCaseCondition | 39 | 9 | pure-ast |
| Lint/DuplicateElsifCondition | 39 | 5 | pure-ast |
| Lint/DuplicateRequire | 56 | 10 | autocorrect |
| Lint/DuplicateRescueException | 47 | 6 | pure-ast |
| Lint/EachWithObjectArgument | 40 | 7 | pure-ast |
| Lint/EmptyConditionalBody | 148 | 42 | tokens/comments, autocorrect, config-options |
| Lint/EmptyEnsure | 48 | 2 | autocorrect |
| Lint/EmptyExpression | 40 | 12 | pure-ast |
| Lint/EmptyFile | 46 | 4 | file-level, config-options |
| Lint/EmptyInterpolation | 42 | 12 | autocorrect |
| Lint/EmptyWhen | 61 | 16 | tokens/comments, config-options |
| Lint/EnsureReturn | 51 | 5 | pure-ast |
| Lint/ErbNewArguments | 162 | 10 | target-ruby, autocorrect |
| Lint/FlipFlop | 38 | 2 | pure-ast |
| Lint/FloatComparison | 136 | 17 | pure-ast |
| Lint/FloatOutOfRange | 28 | 5 | pure-ast |
| Lint/FormatParameterMismatch | 191 | 63 | pure-ast |
| Lint/HashCompareByIdentity | 48 | 4 | pure-ast |
| Lint/IdentityComparison | 54 | 12 | autocorrect |
| Lint/ImplicitStringConcatenation | 112 | 12 | autocorrect, metrics |
| Lint/IneffectiveAccessModifier | 114 | 8 | metrics |
| Lint/InheritException | 105 | 13 | autocorrect |
| Lint/InterpolationCheck | 64 | 12 | autocorrect, metrics |
| Lint/LiteralAsCondition | 283 | 65 | autocorrect, metrics |
| Lint/LiteralInInterpolation | 210 | 38 | autocorrect, metrics |
| Lint/Loop | 80 | 4 | autocorrect |
| Lint/MissingCopEnableDirective | 120 | 11 | file-level, config-options |
| Lint/MixedRegexpCaptureTypes | 37 | 12 | pure-ast |
| Lint/MultipleComparison | 48 | 5 | autocorrect |
| Lint/NestedMethodDefinition | 142 | 38 | pure-ast |
| Lint/NestedPercentLiteral | 63 | 10 | pure-ast |
| Lint/NextWithoutAccumulator | 51 | 8 | pure-ast |
| Lint/NonDeterministicRequireOrder | 183 | 28 | target-ruby, autocorrect |
| Lint/NonLocalExitFromIterator | 86 | 15 | pure-ast |
| Lint/OrderedMagicComments | 81 | 10 | autocorrect, file-level |
| Lint/OutOfRangeRegexpRef | 129 | 56 | file-level |
| Lint/ParenthesesAsGroupedExpression | 87 | 26 | autocorrect |
| Lint/PercentStringArray | 74 | 10 | autocorrect |
| Lint/PercentSymbolArray | 64 | 7 | autocorrect |
| Lint/RaiseException | 110 | 15 | autocorrect, config-options |
| Lint/RandOne | 42 | 2 | pure-ast |
| Lint/RedundantCopEnableDirective | 135 | 23 | tokens/comments, autocorrect, file-level |
| Lint/RedundantRequireStatement | 80 | 15 | target-ruby, autocorrect, metrics |
| Lint/RedundantSafeNavigation | 259 | 70 | autocorrect, config-options |
| Lint/RedundantSplatExpansion | 216 | 40 | autocorrect |
| Lint/RedundantWithIndex | 87 | 17 | autocorrect |
| Lint/RedundantWithObject | 82 | 14 | autocorrect |
| Lint/RegexpAsCondition | 36 | 5 | autocorrect |
| Lint/RequireParentheses | 62 | 16 | pure-ast |
| Lint/RescueException | 38 | 11 | pure-ast |
| Lint/RescueType | 82 | 10 | autocorrect |
| Lint/ReturnInVoidContext | 55 | 19 | pure-ast |
| Lint/SafeNavigationChain | 116 | 42 | target-ruby, autocorrect |
| Lint/SafeNavigationConsistency | 160 | 43 | autocorrect, metrics |
| Lint/SafeNavigationWithEmpty | 46 | 3 | autocorrect |
| Lint/ScriptPermission | 73 | 7 | tokens/comments, autocorrect, file-level |
| Lint/SendWithMixinArgument | 83 | 14 | autocorrect |
| Lint/ShadowedArgument | 177 | 54 | semantic, config-options |
| Lint/StructNewOverride | 59 | 10 | pure-ast |
| Lint/SuppressedException | 132 | 25 | config-options |
| Lint/Syntax | 49 | 7 | pure-ast |
| Lint/ToJSON | 49 | 2 | autocorrect |
| Lint/TopLevelReturnWithArgument | 48 | 10 | autocorrect |
| Lint/TrailingCommaInAttributeDeclaration | 55 | 2 | autocorrect |
| Lint/UnderscorePrefixedVariableName | 80 | 14 | semantic, config-options |
| Lint/UnifiedInteger | 40 | 9 | target-ruby, autocorrect |
| Lint/UnreachableCode | 146 | 32 | pure-ast |
| Lint/UnreachableLoop | 208 | 28 | pure-ast |
| Lint/UnusedBlockArgument | 172 | 30 | semantic, autocorrect, config-options |
| Lint/UnusedMethodArgument | 137 | 41 | semantic, autocorrect, config-options |
| Lint/UriEscapeUnescape | 81 | 9 | pure-ast |
| Lint/UriRegexp | 56 | 10 | target-ruby, autocorrect |
| Lint/UselessElseWithoutRescue | 44 | 2 | target-ruby, file-level |
| Lint/UselessMethodDefinition | 77 | 16 | autocorrect |
| Lint/UselessSetterCall | 158 | 16 | autocorrect |
| Lint/UselessTimes | 114 | 25 | autocorrect |
| Lint/Void | 279 | 98 | autocorrect, metrics, config-options |
| Metrics/AbcSize | 56 | 22 | config-options |
| Metrics/BlockLength | 88 | 36 | metrics |
| Metrics/BlockNesting | 72 | 26 | file-level, config-options |
| Metrics/ClassLength | 77 | 34 | metrics |
| Metrics/CyclomaticComplexity | 58 | 37 | metrics |
| Metrics/MethodLength | 80 | 31 | metrics |
| Metrics/ModuleLength | 62 | 21 | metrics |
| Metrics/ParameterLists | 147 | 16 | config-options |
| Metrics/PerceivedComplexity | 59 | 31 | metrics |
| Migration/DepartmentName | 81 | 8 | tokens/comments, autocorrect, file-level |
| Naming/AccessorMethodName | 78 | 23 | pure-ast |
| Naming/AsciiIdentifiers | 90 | 7 | tokens/comments, file-level, config-options |
| Naming/BinaryOperatorParameterName | 53 | 15 | autocorrect |
| Naming/BlockParameterName | 49 | 13 | pure-ast |
| Naming/ClassAndModuleCamelCase | 45 | 5 | config-options |
| Naming/ConstantName | 82 | 24 | pure-ast |
| Naming/FileName | 245 | 38 | file-level, config-options |
| Naming/HeredocDelimiterCase | 68 | 26 | autocorrect |
| Naming/HeredocDelimiterNaming | 57 | 19 | config-options |
| Naming/MemoizedInstanceVariableName | 294 | 72 | autocorrect |
| Naming/MethodName | 253 | 77 | pure-ast |
| Naming/MethodParameterName | 58 | 23 | pure-ast |
| Naming/PredicatePrefix | 204 | 21 | config-options |
| Naming/RescuedExceptionsVariableName | 172 | 36 | autocorrect |
| Naming/VariableName | 112 | 72 | pure-ast |
| Naming/VariableNumber | 155 | 45 | config-options |
| Security/Eval | 34 | 15 | pure-ast |
| Security/JSONLoad | 0 | 0 | ? |
| Security/MarshalLoad | 39 | 5 | pure-ast |
| Security/Open | 90 | 16 | pure-ast |
| Security/YAMLLoad | 0 | 0 | ? |
| Style/AccessModifierDeclarations | 365 | 51 | tokens/comments, autocorrect, config-options |
| Style/Alias | 158 | 26 | autocorrect |
| Style/AndOr | 158 | 50 | autocorrect |
| Style/ArrayJoin | 39 | 5 | autocorrect |
| Style/Attr | 80 | 11 | autocorrect |
| Style/BarePercentLiterals | 75 | 15 | autocorrect |
| Style/BeginBlock | 21 | 1 | pure-ast |
| Style/BisectedAttrAccessor | 125 | 14 | autocorrect, file-level |
| Style/BlockComments | 66 | 5 | tokens/comments, autocorrect, file-level |
| Style/BlockDelimiters | 503 | 123 | autocorrect, metrics, config-options |
| Style/CaseEquality | 106 | 10 | autocorrect |
| Style/CaseLikeIf | 277 | 38 | autocorrect, metrics |
| Style/CharacterLiteral | 57 | 5 | autocorrect |
| Style/ClassCheck | 55 | 4 | autocorrect |
| Style/ClassEqualityComparison | 134 | 22 | autocorrect |
| Style/ClassMethods | 54 | 5 | autocorrect |
| Style/ClassVars | 64 | 5 | pure-ast |
| Style/ColonMethodCall | 46 | 10 | autocorrect |
| Style/ColonMethodDefinition | 37 | 3 | autocorrect |
| Style/CombinableLoops | 131 | 20 | autocorrect, metrics |
| Style/CommandLiteral | 181 | 35 | autocorrect, config-options |
| Style/CommentAnnotation | 130 | 29 | tokens/comments, autocorrect, file-level, config-options |
| Style/CommentedKeyword | 118 | 32 | tokens/comments, autocorrect, file-level |
| Style/ConditionalAssignment | 670 | 0 | autocorrect, metrics, config-options |
| Style/DefWithParentheses | 70 | 9 | autocorrect |
| Style/Dir | 50 | 4 | target-ruby, autocorrect |
| Style/DoubleCopDisableDirective | 46 | 3 | tokens/comments, autocorrect, file-level |
| Style/DoubleNegation | 159 | 43 | autocorrect |
| Style/EachForSimpleLoop | 86 | 20 | autocorrect |
| Style/EachWithObject | 138 | 15 | autocorrect |
| Style/EmptyBlockParameter | 47 | 9 | autocorrect |
| Style/EmptyCaseCondition | 117 | 11 | tokens/comments, autocorrect, metrics |
| Style/EmptyLambdaParameter | 44 | 3 | autocorrect |
| Style/EmptyLiteral | 151 | 40 | autocorrect |
| Style/EmptyMethod | 113 | 32 | autocorrect |
| Style/Encoding | 63 | 13 | autocorrect, file-level |
| Style/EndBlock | 28 | 2 | autocorrect |
| Style/EvalWithLocation | 229 | 27 | autocorrect |
| Style/EvenOdd | 56 | 18 | autocorrect |
| Style/ExpandPathArguments | 191 | 16 | autocorrect |
| Style/ExplicitBlockArgument | 166 | 21 | autocorrect |
| Style/ExponentialNotation | 117 | 27 | pure-ast |
| Style/FloatDivision | 169 | 31 | autocorrect |
| Style/For | 90 | 32 | autocorrect |
| Style/FormatString | 154 | 46 | autocorrect |
| Style/FormatStringToken | 255 | 42 | autocorrect, config-options |
| Style/GlobalStdStream | 79 | 6 | autocorrect |
| Style/GlobalVars | 78 | 4 | config-options |
| Style/HashAsLastArrayItem | 100 | 15 | autocorrect |
| Style/HashEachMethods | 221 | 62 | autocorrect |
| Style/HashLikeCase | 71 | 8 | pure-ast |
| Style/HashTransformKeys | 95 | 40 | target-ruby, autocorrect |
| Style/HashTransformValues | 93 | 40 | target-ruby, autocorrect |
| Style/IdenticalConditionalBranches | 273 | 48 | autocorrect, metrics |
| Style/IfInsideElse | 152 | 21 | autocorrect, metrics, config-options |
| Style/IfWithSemicolon | 132 | 28 | autocorrect |
| Style/InfiniteLoop | 127 | 17 | semantic, autocorrect |
| Style/InverseMethods | 200 | 40 | autocorrect, config-options |
| Style/KeywordParametersOrder | 81 | 10 | autocorrect |
| Style/Lambda | 126 | 41 | autocorrect |
| Style/LambdaCall | 79 | 19 | autocorrect |
| Style/LineEndConcatenation | 143 | 19 | tokens/comments, autocorrect, file-level |
| Style/MethodCallWithoutArgsParentheses | 120 | 40 | autocorrect, metrics |
| Style/MethodDefParentheses | 180 | 25 | autocorrect |
| Style/MinMax | 64 | 12 | autocorrect |
| Style/MissingRespondToMissing | 82 | 8 | pure-ast |
| Style/MixinGrouping | 135 | 18 | autocorrect |
| Style/MixinUsage | 72 | 18 | pure-ast |
| Style/ModuleFunction | 170 | 11 | autocorrect |
| Style/MultilineBlockChain | 51 | 13 | pure-ast |
| Style/MultilineIfModifier | 58 | 10 | autocorrect |
| Style/MultilineIfThen | 44 | 11 | autocorrect |
| Style/MultilineMemoization | 96 | 11 | autocorrect |
| Style/MultilineTernaryOperator | 102 | 17 | tokens/comments, autocorrect |
| Style/MultilineWhenThen | 60 | 13 | autocorrect |
| Style/MultipleComparison | 164 | 34 | autocorrect, metrics |
| Style/NegatedIf | 98 | 15 | autocorrect |
| Style/NegatedUnless | 88 | 14 | autocorrect |
| Style/NegatedWhile | 40 | 9 | autocorrect |
| Style/NestedModifier | 100 | 11 | autocorrect |
| Style/NestedParenthesizedCalls | 79 | 12 | autocorrect |
| Style/NestedTernaryOperator | 63 | 7 | autocorrect |
| Style/Next | 277 | 44 | autocorrect |
| Style/NilComparison | 87 | 8 | autocorrect |
| Style/NonNilCheck | 158 | 21 | autocorrect, config-options |
| Style/Not | 76 | 9 | autocorrect |
| Style/NumericPredicate | 185 | 39 | target-ruby, autocorrect |
| Style/OneLineConditional | 156 | 38 | autocorrect, config-options |
| Style/OptionalArguments | 59 | 12 | pure-ast |
| Style/OrAssignment | 94 | 22 | autocorrect |
| Style/ParallelAssignment | 302 | 55 | autocorrect |
| Style/ParenthesesAroundCondition | 136 | 26 | autocorrect, config-options |
| Style/PercentLiteralDelimiters | 118 | 49 | autocorrect |
| Style/PercentQLiterals | 0 | 0 | ? |
| Style/PerlBackrefs | 127 | 14 | autocorrect |
| Style/PreferredHashMethods | 74 | 9 | autocorrect |
| Style/Proc | 37 | 6 | autocorrect |
| Style/RaiseArgs | 160 | 35 | autocorrect, config-options |
| Style/RandomWithOffset | 153 | 29 | autocorrect |
| Style/RedundantAssignment | 114 | 11 | autocorrect, metrics |
| Style/RedundantBegin | 239 | 65 | target-ruby, autocorrect |
| Style/RedundantCapitalW | 46 | 13 | autocorrect |
| Style/RedundantConditional | 84 | 11 | autocorrect |
| Style/RedundantException | 85 | 12 | autocorrect |
| Style/RedundantFetchBlock | 113 | 15 | autocorrect, config-options |
| Style/RedundantFileExtensionInRequire | 61 | 4 | autocorrect |
| Style/RedundantFreeze | 69 | 3 | target-ruby, autocorrect |
| Style/RedundantInterpolation | 147 | 29 | target-ruby, autocorrect |
| Style/RedundantPercentQ | 107 | 25 | autocorrect |
| Style/RedundantSelfAssignment | 106 | 14 | autocorrect |
| Style/RedundantSort | 209 | 50 | autocorrect |
| Style/RedundantSortBy | 79 | 8 | autocorrect |
| Style/RegexpLiteral | 231 | 57 | autocorrect, config-options |
| Style/RescueModifier | 112 | 21 | autocorrect |
| Style/RescueStandardError | 126 | 37 | autocorrect |
| Style/SafeNavigation | 427 | 170 | tokens/comments, target-ruby, autocorrect, metrics, config-options |
| Style/Sample | 144 | 3 | autocorrect, metrics |
| Style/SelfAssignment | 95 | 3 | autocorrect |
| Style/Semicolon | 188 | 33 | tokens/comments, autocorrect, file-level, metrics, config-options |
| Style/SignalException | 217 | 27 | autocorrect |
| Style/SingleArgumentDig | 73 | 15 | autocorrect |
| Style/SingleLineMethods | 147 | 37 | target-ruby, autocorrect, config-options |
| Style/SlicingWithRange | 146 | 28 | target-ruby, autocorrect |
| Style/SpecialGlobalVars | 259 | 31 | autocorrect, file-level, config-options |
| Style/StabbyLambdaParentheses | 79 | 6 | autocorrect |
| Style/StderrPuts | 57 | 5 | autocorrect |
| Style/StringLiteralsInInterpolation | 76 | 13 | autocorrect |
| Style/Strip | 45 | 6 | autocorrect |
| Style/StructInheritance | 79 | 12 | autocorrect |
| Style/SymbolArray | 133 | 33 | target-ruby, autocorrect |
| Style/SymbolLiteral | 28 | 4 | autocorrect |
| Style/TernaryParentheses | 240 | 95 | target-ruby, autocorrect |
| Style/TrailingBodyOnClass | 41 | 7 | autocorrect |
| Style/TrailingBodyOnMethodDefinition | 55 | 12 | autocorrect |
| Style/TrailingBodyOnModule | 40 | 7 | autocorrect |
| Style/TrailingMethodEndStatement | 62 | 10 | autocorrect |
| Style/TrailingUnderscoreVariable | 152 | 34 | autocorrect, config-options |
| Style/TrivialAccessors | 254 | 38 | autocorrect, config-options |
| Style/UnlessElse | 56 | 5 | autocorrect |
| Style/UnpackFirst | 59 | 11 | target-ruby, autocorrect |
| Style/VariableInterpolation | 44 | 9 | autocorrect |
| Style/WhenThen | 37 | 4 | autocorrect |
| Style/WhileUntilDo | 47 | 6 | autocorrect |
| Style/WhileUntilModifier | 51 | 0 | autocorrect |
| Style/YodaCondition | 185 | 69 | autocorrect, metrics |
| Style/ZeroLengthPredicate | 154 | 59 | autocorrect |
