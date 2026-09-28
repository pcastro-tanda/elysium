# Default-cop parity inventory
Generated from RuboCop 1.82.1 `config/default.yml` (`Enabled: true` only; `pending` cops excluded) minus the rules registered in `docs/rules/`. Regenerate with the script in this file's git history / `tools/` once ported there.
**229 default-enabled cops missing of 394** (165 implemented).
## By department
- Style: 117
- Layout: 49
- Lint: 44
- Naming: 8
- Metrics: 8
- Bundler: 1
- Gemspec: 0
- Security: 2
- Migration: 0

## By required infrastructure
Coarse, from mixins/API usage in the cop source. `pure-ast` = only node callbacks + `add_offense`. `tokens/comments` = needs the token stream or comment list (we have directives only). `target-ruby` = branches on `TargetRubyVersion`. `metrics` = needs code-length/complexity utilities. `semantic` = uses `VariableForce`.
- autocorrect: 257
- config-options: 52
- file-level: 39
- metrics: 30
- tokens/comments: 26
- target-ruby: 25
- semantic: 5
- ?: 4
- pure-ast: 1

## Full list
| Cop | src lines | spec `it`s | needs |
|---|---|---|---|
| Bundler/InsecureProtocolSource | 85 | 6 | autocorrect |
| Layout/AccessModifierIndentation | 104 | 41 | autocorrect |
| Layout/ArrayAlignment | 84 | 25 | autocorrect, config-options |
| Layout/AssignmentIndentation | 57 | 10 | autocorrect |
| Layout/BeginEndAlignment | 73 | 0 | autocorrect |
| Layout/BlockAlignment | 259 | 79 | autocorrect |
| Layout/BlockEndNewline | 83 | 22 | autocorrect |
| Layout/CaseIndentation | 219 | 50 | autocorrect, config-options |
| Layout/ClosingHeredocIndentation | 123 | 11 | autocorrect |
| Layout/ClosingParenthesisIndentation | 193 | 43 | autocorrect |
| Layout/ConditionPosition | 60 | 5 | autocorrect |
| Layout/DefEndAlignment | 73 | 4 | autocorrect |
| Layout/DotPosition | 139 | 39 | autocorrect |
| Layout/ElseAlignment | 156 | 53 | autocorrect |
| Layout/EmptyLineAfterGuardClause | 217 | 47 | autocorrect, metrics |
| Layout/EmptyLinesAroundAccessModifier | 241 | 48 | autocorrect |
| Layout/EmptyLinesAroundArguments | 82 | 22 | autocorrect |
| Layout/EmptyLinesAroundAttributeAccessor | 139 | 20 | autocorrect |
| Layout/EmptyLinesAroundBeginBody | 41 | 10 | autocorrect |
| Layout/EmptyLinesAroundBlockBody | 41 | 12 | autocorrect |
| Layout/EmptyLinesAroundExceptionHandlingKeywords | 139 | 13 | autocorrect |
| Layout/EmptyLinesAroundMethodBody | 64 | 14 | autocorrect |
| Layout/EmptyLinesAroundModuleBody | 59 | 18 | autocorrect |
| Layout/EndAlignment | 214 | 14 | autocorrect |
| Layout/FirstArrayElementIndentation | 189 | 53 | autocorrect |
| Layout/FirstParameterIndentation | 101 | 20 | autocorrect |
| Layout/LeadingCommentSpace | 203 | 27 | tokens/comments, autocorrect, file-level, metrics, config-options |
| Layout/MultilineArrayBraceLayout | 115 | 4 | autocorrect |
| Layout/MultilineBlockLayout | 164 | 32 | autocorrect |
| Layout/MultilineHashBraceLayout | 115 | 3 | autocorrect |
| Layout/MultilineMethodCallBraceLayout | 132 | 15 | autocorrect |
| Layout/MultilineMethodCallIndentation | 267 | 120 | autocorrect, config-options |
| Layout/MultilineMethodDefinitionBraceLayout | 128 | 3 | autocorrect |
| Layout/MultilineOperationIndentation | 130 | 60 | autocorrect, config-options |
| Layout/ParameterAlignment | 118 | 19 | autocorrect, config-options |
| Layout/SpaceAfterColon | 49 | 12 | autocorrect |
| Layout/SpaceAfterComma | 32 | 5 | autocorrect |
| Layout/SpaceAfterMethodName | 39 | 8 | autocorrect |
| Layout/SpaceAfterNot | 39 | 6 | autocorrect |
| Layout/SpaceAfterSemicolon | 39 | 7 | autocorrect |
| Layout/SpaceAroundBlockParameters | 162 | 42 | autocorrect |
| Layout/SpaceAroundMethodCallOperator | 98 | 17 | autocorrect |
| Layout/SpaceBeforeBlockBraces | 164 | 20 | autocorrect, config-options |
| Layout/SpaceBeforeComma | 29 | 6 | autocorrect |
| Layout/SpaceBeforeFirstArg | 73 | 12 | autocorrect |
| Layout/SpaceBeforeSemicolon | 24 | 8 | autocorrect |
| Layout/SpaceInLambdaLiteral | 78 | 15 | autocorrect |
| Layout/SpaceInsideArrayPercentLiteral | 46 | 9 | autocorrect |
| Layout/SpaceInsidePercentLiteralDelimiters | 94 | 15 | autocorrect |
| Layout/SpaceInsideRangeLiteral | 54 | 7 | autocorrect |
| Lint/AssignmentInCondition | 107 | 37 | autocorrect |
| Lint/BigDecimalNew | 41 | 3 | autocorrect |
| Lint/BooleanSymbol | 61 | 10 | autocorrect |
| Lint/DeprecatedClassMethods | 118 | 31 | autocorrect |
| Lint/DeprecatedOpenSSLConstant | 0 | 0 | ? |
| Lint/DisjunctiveAssignmentInConstructor | 110 | 7 | autocorrect |
| Lint/DuplicateRequire | 56 | 10 | autocorrect |
| Lint/EmptyInterpolation | 42 | 12 | autocorrect |
| Lint/IdentityComparison | 54 | 12 | autocorrect |
| Lint/ImplicitStringConcatenation | 112 | 12 | autocorrect, metrics |
| Lint/IneffectiveAccessModifier | 114 | 8 | metrics |
| Lint/InheritException | 105 | 13 | autocorrect |
| Lint/InterpolationCheck | 64 | 12 | autocorrect, metrics |
| Lint/LiteralAsCondition | 283 | 65 | autocorrect, metrics |
| Lint/LiteralInInterpolation | 210 | 38 | autocorrect, metrics |
| Lint/Loop | 80 | 4 | autocorrect |
| Lint/MultipleComparison | 48 | 5 | autocorrect |
| Lint/ParenthesesAsGroupedExpression | 87 | 26 | autocorrect |
| Lint/PercentStringArray | 74 | 10 | autocorrect |
| Lint/PercentSymbolArray | 64 | 7 | autocorrect |
| Lint/RaiseException | 110 | 15 | autocorrect, config-options |
| Lint/RedundantRequireStatement | 80 | 15 | target-ruby, autocorrect, metrics |
| Lint/RedundantSafeNavigation | 259 | 70 | autocorrect, config-options |
| Lint/RedundantSplatExpansion | 216 | 40 | autocorrect |
| Lint/RedundantWithIndex | 87 | 17 | autocorrect |
| Lint/RedundantWithObject | 82 | 14 | autocorrect |
| Lint/RegexpAsCondition | 36 | 5 | autocorrect |
| Lint/RescueType | 82 | 10 | autocorrect |
| Lint/SafeNavigationConsistency | 160 | 43 | autocorrect, metrics |
| Lint/SafeNavigationWithEmpty | 46 | 3 | autocorrect |
| Lint/SendWithMixinArgument | 83 | 14 | autocorrect |
| Lint/ShadowedArgument | 177 | 54 | semantic, config-options |
| Lint/SuppressedException | 132 | 25 | config-options |
| Lint/Syntax | 49 | 7 | pure-ast |
| Lint/ToJSON | 49 | 2 | autocorrect |
| Lint/TopLevelReturnWithArgument | 48 | 10 | autocorrect |
| Lint/TrailingCommaInAttributeDeclaration | 55 | 2 | autocorrect |
| Lint/UnderscorePrefixedVariableName | 80 | 14 | semantic, config-options |
| Lint/UnusedBlockArgument | 172 | 30 | semantic, autocorrect, config-options |
| Lint/UnusedMethodArgument | 137 | 41 | semantic, autocorrect, config-options |
| Lint/UselessMethodDefinition | 77 | 16 | autocorrect |
| Lint/UselessSetterCall | 158 | 16 | autocorrect |
| Lint/UselessTimes | 114 | 25 | autocorrect |
| Lint/Void | 279 | 98 | autocorrect, metrics, config-options |
| Metrics/AbcSize | 56 | 22 | config-options |
| Metrics/BlockLength | 88 | 36 | metrics |
| Metrics/ClassLength | 77 | 34 | metrics |
| Metrics/CyclomaticComplexity | 58 | 37 | metrics |
| Metrics/MethodLength | 80 | 31 | metrics |
| Metrics/ModuleLength | 62 | 21 | metrics |
| Metrics/ParameterLists | 147 | 16 | config-options |
| Metrics/PerceivedComplexity | 59 | 31 | metrics |
| Naming/BinaryOperatorParameterName | 53 | 15 | autocorrect |
| Naming/ClassAndModuleCamelCase | 45 | 5 | config-options |
| Naming/HeredocDelimiterCase | 68 | 26 | autocorrect |
| Naming/HeredocDelimiterNaming | 57 | 19 | config-options |
| Naming/MemoizedInstanceVariableName | 294 | 72 | autocorrect |
| Naming/PredicatePrefix | 204 | 21 | config-options |
| Naming/RescuedExceptionsVariableName | 172 | 36 | autocorrect |
| Naming/VariableNumber | 155 | 45 | config-options |
| Security/JSONLoad | 0 | 0 | ? |
| Security/YAMLLoad | 0 | 0 | ? |
| Style/Alias | 158 | 26 | autocorrect |
| Style/AndOr | 158 | 50 | autocorrect |
| Style/ArrayJoin | 39 | 5 | autocorrect |
| Style/Attr | 80 | 11 | autocorrect |
| Style/BarePercentLiterals | 75 | 15 | autocorrect |
| Style/BlockDelimiters | 503 | 123 | autocorrect, metrics, config-options |
| Style/CaseEquality | 106 | 10 | autocorrect |
| Style/CaseLikeIf | 277 | 38 | autocorrect, metrics |
| Style/CharacterLiteral | 57 | 5 | autocorrect |
| Style/ClassCheck | 55 | 4 | autocorrect |
| Style/ClassEqualityComparison | 134 | 22 | autocorrect |
| Style/ClassMethods | 54 | 5 | autocorrect |
| Style/ColonMethodCall | 46 | 10 | autocorrect |
| Style/ColonMethodDefinition | 37 | 3 | autocorrect |
| Style/CombinableLoops | 131 | 20 | autocorrect, metrics |
| Style/CommandLiteral | 181 | 35 | autocorrect, config-options |
| Style/ConditionalAssignment | 670 | 0 | autocorrect, metrics, config-options |
| Style/DefWithParentheses | 70 | 9 | autocorrect |
| Style/DoubleCopDisableDirective | 46 | 3 | tokens/comments, autocorrect, file-level |
| Style/DoubleNegation | 159 | 43 | autocorrect |
| Style/EachForSimpleLoop | 86 | 20 | autocorrect |
| Style/EachWithObject | 138 | 15 | autocorrect |
| Style/EmptyBlockParameter | 47 | 9 | autocorrect |
| Style/EmptyCaseCondition | 117 | 11 | tokens/comments, autocorrect, metrics |
| Style/EmptyLambdaParameter | 44 | 3 | autocorrect |
| Style/EmptyLiteral | 151 | 40 | autocorrect |
| Style/EmptyMethod | 113 | 32 | autocorrect |
| Style/EndBlock | 28 | 2 | autocorrect |
| Style/EvalWithLocation | 229 | 27 | autocorrect |
| Style/EvenOdd | 56 | 18 | autocorrect |
| Style/ExpandPathArguments | 191 | 16 | autocorrect |
| Style/ExplicitBlockArgument | 166 | 21 | autocorrect |
| Style/FloatDivision | 169 | 31 | autocorrect |
| Style/For | 90 | 32 | autocorrect |
| Style/FormatString | 154 | 46 | autocorrect |
| Style/FormatStringToken | 255 | 42 | autocorrect, config-options |
| Style/GlobalStdStream | 79 | 6 | autocorrect |
| Style/GlobalVars | 78 | 4 | config-options |
| Style/HashAsLastArrayItem | 100 | 15 | autocorrect |
| Style/HashEachMethods | 221 | 62 | autocorrect |
| Style/IdenticalConditionalBranches | 273 | 48 | autocorrect, metrics |
| Style/IfInsideElse | 152 | 21 | autocorrect, metrics, config-options |
| Style/IfWithSemicolon | 132 | 28 | autocorrect |
| Style/InfiniteLoop | 127 | 17 | semantic, autocorrect |
| Style/InverseMethods | 200 | 40 | autocorrect, config-options |
| Style/KeywordParametersOrder | 81 | 10 | autocorrect |
| Style/Lambda | 126 | 41 | autocorrect |
| Style/LambdaCall | 79 | 19 | autocorrect |
| Style/MethodCallWithoutArgsParentheses | 120 | 40 | autocorrect, metrics |
| Style/MethodDefParentheses | 180 | 25 | autocorrect |
| Style/MinMax | 64 | 12 | autocorrect |
| Style/MixinGrouping | 135 | 18 | autocorrect |
| Style/ModuleFunction | 170 | 11 | autocorrect |
| Style/MultilineIfModifier | 58 | 10 | autocorrect |
| Style/MultilineIfThen | 44 | 11 | autocorrect |
| Style/MultilineMemoization | 96 | 11 | autocorrect |
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
| Style/OneLineConditional | 156 | 38 | autocorrect, config-options |
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
| Style/RedundantCapitalW | 46 | 13 | autocorrect |
| Style/RedundantConditional | 84 | 11 | autocorrect |
| Style/RedundantException | 85 | 12 | autocorrect |
| Style/RedundantFetchBlock | 113 | 15 | autocorrect, config-options |
| Style/RedundantFileExtensionInRequire | 61 | 4 | autocorrect |
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
| Style/StabbyLambdaParentheses | 79 | 6 | autocorrect |
| Style/StderrPuts | 57 | 5 | autocorrect |
| Style/StringLiteralsInInterpolation | 76 | 13 | autocorrect |
| Style/Strip | 45 | 6 | autocorrect |
| Style/StructInheritance | 79 | 12 | autocorrect |
| Style/SymbolLiteral | 28 | 4 | autocorrect |
| Style/TrailingBodyOnClass | 41 | 7 | autocorrect |
| Style/TrailingBodyOnMethodDefinition | 55 | 12 | autocorrect |
| Style/TrailingBodyOnModule | 40 | 7 | autocorrect |
| Style/TrailingMethodEndStatement | 62 | 10 | autocorrect |
| Style/TrailingUnderscoreVariable | 152 | 34 | autocorrect, config-options |
| Style/TrivialAccessors | 254 | 38 | autocorrect, config-options |
| Style/UnlessElse | 56 | 5 | autocorrect |
| Style/VariableInterpolation | 44 | 9 | autocorrect |
| Style/WhenThen | 37 | 4 | autocorrect |
| Style/WhileUntilDo | 47 | 6 | autocorrect |
| Style/WhileUntilModifier | 51 | 0 | autocorrect |
| Style/YodaCondition | 185 | 69 | autocorrect, metrics |
| Style/ZeroLengthPredicate | 154 | 59 | autocorrect |
