# Default-cop parity inventory
Generated from RuboCop 1.82.1 `config/default.yml` (`Enabled: true` only; `pending` cops excluded) minus the rules registered in `docs/rules/`. Regenerate with the script in this file's git history / `tools/` once ported there.
**110 default-enabled cops missing of 393** (283 implemented). `Style/DoubleCopDisableDirective` is excluded: RuboCop 1.91.0, the corpus truth, removed it.
## By department
- Style: 52
- Layout: 49
- Lint: 1
- Naming: 0
- Metrics: 8
- Bundler: 0
- Gemspec: 0
- Security: 0
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
| Lint/Syntax | 49 | 7 | pure-ast |
| Metrics/AbcSize | 56 | 22 | config-options |
| Metrics/BlockLength | 88 | 36 | metrics |
| Metrics/ClassLength | 77 | 34 | metrics |
| Metrics/CyclomaticComplexity | 58 | 37 | metrics |
| Metrics/MethodLength | 80 | 31 | metrics |
| Metrics/ModuleLength | 62 | 21 | metrics |
| Metrics/ParameterLists | 147 | 16 | config-options |
| Metrics/PerceivedComplexity | 59 | 31 | metrics |
| Style/Alias | 158 | 26 | autocorrect |
| Style/AndOr | 158 | 50 | autocorrect |
| Style/BlockDelimiters | 503 | 123 | autocorrect, metrics, config-options |
| Style/CaseLikeIf | 277 | 38 | autocorrect, metrics |
| Style/ClassEqualityComparison | 134 | 22 | autocorrect |
| Style/CombinableLoops | 131 | 20 | autocorrect, metrics |
| Style/CommandLiteral | 181 | 35 | autocorrect, config-options |
| Style/ConditionalAssignment | 670 | 0 | autocorrect, metrics, config-options |
| Style/DoubleNegation | 159 | 43 | autocorrect |
| Style/EachWithObject | 138 | 15 | autocorrect |
| Style/EmptyCaseCondition | 117 | 11 | tokens/comments, autocorrect, metrics |
| Style/EmptyLiteral | 151 | 40 | autocorrect |
| Style/EvalWithLocation | 229 | 27 | autocorrect |
| Style/ExpandPathArguments | 191 | 16 | autocorrect |
| Style/ExplicitBlockArgument | 166 | 21 | autocorrect |
| Style/FloatDivision | 169 | 31 | autocorrect |
| Style/FormatString | 154 | 46 | autocorrect |
| Style/FormatStringToken | 255 | 42 | autocorrect, config-options |
| Style/HashEachMethods | 221 | 62 | autocorrect |
| Style/IdenticalConditionalBranches | 273 | 48 | autocorrect, metrics |
| Style/IfInsideElse | 152 | 21 | autocorrect, metrics, config-options |
| Style/IfWithSemicolon | 132 | 28 | autocorrect |
| Style/InfiniteLoop | 127 | 17 | semantic, autocorrect |
| Style/InverseMethods | 200 | 40 | autocorrect, config-options |
| Style/Lambda | 126 | 41 | autocorrect |
| Style/MethodCallWithoutArgsParentheses | 120 | 40 | autocorrect, metrics |
| Style/MethodDefParentheses | 180 | 25 | autocorrect |
| Style/MixinGrouping | 135 | 18 | autocorrect |
| Style/ModuleFunction | 170 | 11 | autocorrect |
| Style/MultipleComparison | 164 | 34 | autocorrect, metrics |
| Style/Next | 277 | 44 | autocorrect |
| Style/NonNilCheck | 158 | 21 | autocorrect, config-options |
| Style/OneLineConditional | 156 | 38 | autocorrect, config-options |
| Style/ParallelAssignment | 302 | 55 | autocorrect |
| Style/ParenthesesAroundCondition | 136 | 26 | autocorrect, config-options |
| Style/PercentLiteralDelimiters | 118 | 49 | autocorrect |
| Style/PerlBackrefs | 127 | 14 | autocorrect |
| Style/RaiseArgs | 160 | 35 | autocorrect, config-options |
| Style/RandomWithOffset | 153 | 29 | autocorrect |
| Style/RedundantAssignment | 114 | 11 | autocorrect, metrics |
| Style/RedundantFetchBlock | 113 | 15 | autocorrect, config-options |
| Style/RedundantSort | 209 | 50 | autocorrect |
| Style/RegexpLiteral | 231 | 57 | autocorrect, config-options |
| Style/RescueStandardError | 126 | 37 | autocorrect |
| Style/SafeNavigation | 427 | 170 | tokens/comments, target-ruby, autocorrect, metrics, config-options |
| Style/Sample | 144 | 3 | autocorrect, metrics |
| Style/Semicolon | 188 | 33 | tokens/comments, autocorrect, file-level, metrics, config-options |
| Style/SignalException | 217 | 27 | autocorrect |
| Style/TrailingUnderscoreVariable | 152 | 34 | autocorrect, config-options |
| Style/TrivialAccessors | 254 | 38 | autocorrect, config-options |
| Style/YodaCondition | 185 | 69 | autocorrect, metrics |
| Style/ZeroLengthPredicate | 154 | 59 | autocorrect |
